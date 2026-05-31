// ============================================================
// 系统依赖检测 — Python / FFmpeg / yt-dlp / GPU
// 优先使用配置中的 python_path，再回退 PATH 探测
// Python 版本校验 >= 3.9
// ============================================================

use crate::commands::python::{python_command_parts, resolve_python_path};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Python 最低版本要求
const PYTHON_MIN_VERSION: (u32, u32) = (3, 9);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DepCheckResult {
    pub name: String,
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AllDepResults {
    pub python: DepCheckResult,
    pub ffmpeg: DepCheckResult,
    #[serde(rename = "faster-whisper")]
    pub faster_whisper: DepCheckResult,
    #[serde(rename = "yt-dlp")]
    pub ytdlp: DepCheckResult,
    pub gpu: DepCheckResult,
}

/// 检测 Python — 使用配置路径或自动探测
///
/// 优先级:
/// 1. 用户配置的 python_path（参数传入）
/// 2. faster-whisper venv 的 Python
/// 3. 系统 PATH 中的 python/python3
#[tauri::command]
pub async fn detect_python(python_path: Option<String>) -> Result<DepCheckResult, AppError> {
    let py = resolve_python_path(python_path.as_deref());
    probe_python(&py)
}

/// 检测 Python — 无参数版本（仅探测系统 PATH + venv）
#[tauri::command]
pub async fn detect_python_auto() -> Result<DepCheckResult, AppError> {
    let py = resolve_python_path(None);
    probe_python(&py)
}

/// 探测指定 Python 路径，校验版本 >= 3.9
fn probe_python(python_cmd: &str) -> Result<DepCheckResult, AppError> {
    let (program, prefix_args) = python_command_parts(python_cmd);
    let output = match Command::new(program)
        .args(prefix_args)
        .args(["--version"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => {
            return Ok(DepCheckResult {
                name: "Python".into(),
                found: false,
                path: None,
                version: None,
                suggestion: Some("请安装 Python 3.9+: https://www.python.org/downloads/".into()),
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let version_str = stdout
        .strip_prefix("Python ")
        .unwrap_or(&stdout)
        .to_string();

    // 解析版本号 "3.12.0" → (3, 12, 0)
    let parsed = parse_python_version(&version_str);

    match parsed {
        Some((major, minor, _)) if (major, minor) >= PYTHON_MIN_VERSION => Ok(DepCheckResult {
            name: "Python".into(),
            found: true,
            path: Some(python_cmd.to_string()),
            version: Some(version_str),
            suggestion: None,
        }),
        Some((major, minor, _)) => Ok(DepCheckResult {
            name: "Python".into(),
            found: false,
            path: Some(python_cmd.to_string()),
            version: Some(version_str),
            suggestion: Some(format!(
                "Python 版本过低: {}.{}，需要 >= {}.{}",
                major, minor, PYTHON_MIN_VERSION.0, PYTHON_MIN_VERSION.1
            )),
        }),
        None => {
            // 版本号解析失败，但 Python 可运行 — 给出警告但标记为 found
            Ok(DepCheckResult {
                name: "Python".into(),
                found: true,
                path: Some(python_cmd.to_string()),
                version: Some(version_str),
                suggestion: Some("无法解析版本号，请确认 Python >= 3.9".into()),
            })
        }
    }
}

/// 解析 Python 版本号 "3.12.0" → Some((3, 12, 0))
fn parse_python_version(v: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

/// 检测 FFmpeg
#[tauri::command]
pub async fn detect_ffmpeg() -> Result<DepCheckResult, AppError> {
    let cmd = resolve_ffmpeg_binary("ffmpeg").unwrap_or_else(|| {
        if cfg!(target_os = "windows") {
            "ffmpeg.exe".into()
        } else {
            "ffmpeg".into()
        }
    });

    match Command::new(&cmd).args(["-version"]).output() {
        Ok(output) if output.status.success() => {
            let line = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("unknown")
                .to_string();
            let version = line
                .strip_prefix("ffmpeg version ")
                .and_then(|v| v.split_whitespace().next())
                .unwrap_or("unknown")
                .to_string();

            Ok(DepCheckResult {
                name: "FFmpeg".into(),
                found: true,
                path: Some(cmd.to_string()),
                version: Some(version),
                suggestion: None,
            })
        }
        _ => Ok(DepCheckResult {
            name: "FFmpeg".into(),
            found: false,
            path: None,
            version: None,
            suggestion: Some(
                "请安装 FFmpeg：winget install Gyan.FFmpeg，或把 ffmpeg.exe 加入 PATH".into(),
            ),
        }),
    }
}

fn resolve_ffmpeg_binary(name: &str) -> Option<String> {
    let candidates = if cfg!(target_os = "windows") {
        vec![format!("{name}.exe"), name.to_string()]
    } else {
        vec![name.to_string()]
    };

    for candidate in candidates {
        if Command::new(&candidate)
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(candidate);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_base = PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if let Ok(entries) = std::fs::read_dir(winget_base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let pkg_name = entry.file_name().to_string_lossy().to_lowercase();
                    if !pkg_name.contains("ffmpeg") {
                        continue;
                    }
                    for nested in [
                        path.join("bin").join(format!("{name}.exe")),
                        path.join(format!("{name}.exe")),
                    ] {
                        if nested.exists() {
                            return Some(nested.to_string_lossy().to_string());
                        }
                    }
                    if let Ok(walk) = std::fs::read_dir(&path) {
                        for child in walk.flatten() {
                            let exe = child.path().join(format!("{name}.exe"));
                            if exe.exists() {
                                return Some(exe.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// 检测 yt-dlp（支持直接调用和 python -m 两种方式）
#[tauri::command]
pub async fn detect_ytdlp(python_path: Option<String>) -> Result<DepCheckResult, AppError> {
    // 快速检查：只要能启动 yt-dlp 就算检测到（部分版本 --version 输出到 stderr）
    let check = |cmd: &str, args: &[&str]| -> Option<(String, String)> {
        match Command::new(cmd).args(args).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                // yt-dlp 通常把版本信息打印到 stderr+stdout 混合
                let combined = format!("{stdout} {stderr}").trim().to_string();
                if (!combined.is_empty() && combined.contains("yt-dlp")) || output.status.success()
                {
                    // 提取版本号：形如 "2025.05.31" 或 "2025.05.31.123456"
                    let version = stdout
                        .lines()
                        .chain(stderr.lines())
                        .find(|l| l.contains("yt-dlp"))
                        .map(|l| l.trim().to_string())
                        .unwrap_or_else(|| combined);
                    Some((cmd.to_string(), version))
                } else {
                    None
                }
            }
            Err(e) => {
                log::debug!("[deps] yt-dlp check failed for {cmd}: {e}");
                None
            }
        }
    };

    // 方式 1: 直接调用
    for cmd_name in &["yt-dlp", "yt-dlp.exe"] {
        if let Some((path, ver)) = check(cmd_name, &["--version"]) {
            return Ok(DepCheckResult {
                name: "yt-dlp".into(),
                found: true,
                path: Some(path),
                version: Some(ver),
                suggestion: None,
            });
        }
    }

    // 方式 2: python -m yt_dlp
    let py = resolve_python_path(python_path.as_deref());
    let (py_program, mut py_args) = python_command_parts(&py);
    py_args.extend(["-m".into(), "yt_dlp".into(), "--version".into()]);
    let py_args_refs: Vec<&str> = py_args.iter().map(String::as_str).collect();
    if let Some((_path, ver)) = check(&py_program, &py_args_refs) {
        return Ok(DepCheckResult {
            name: "yt-dlp".into(),
            found: true,
            path: Some(py),
            version: Some(ver),
            suggestion: Some("yt-dlp 通过 Python 模块可用".into()),
        });
    }

    // 方式 3: 用 where/which 找位置
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("where").args(["yt-dlp"]).output() {
            let loc = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !loc.is_empty() {
                return Ok(DepCheckResult {
                    name: "yt-dlp".into(),
                    found: true,
                    path: Some(loc.lines().next().unwrap_or("yt-dlp").to_string()),
                    version: Some("已安装".into()),
                    suggestion: None,
                });
            }
        }
    }

    Ok(DepCheckResult {
        name: "yt-dlp".into(),
        found: false,
        path: None,
        version: None,
        suggestion: Some(
            "winget install yt-dlp.yt-dlp，或在配置的 Python 中执行 pip install -U yt-dlp".into(),
        ),
    })
}

/// 检测 faster-whisper（使用当前配置的 Python / venv）
#[tauri::command]
pub async fn detect_faster_whisper(
    python_path: Option<String>,
) -> Result<DepCheckResult, AppError> {
    let py = resolve_python_path(python_path.as_deref());
    let (program, mut args) = python_command_parts(&py);
    args.extend([
        "-c".into(),
        "import faster_whisper, ctranslate2; print(getattr(faster_whisper, '__version__', 'unknown'))".into(),
    ]);

    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            Ok(DepCheckResult {
                name: "faster-whisper".into(),
                found: true,
                path: Some(py),
                version: Some(if version.is_empty() { "已安装".into() } else { version }),
                suggestion: None,
            })
        }
        Ok(output) => Ok(DepCheckResult {
            name: "faster-whisper".into(),
            found: false,
            path: Some(py),
            version: None,
            suggestion: Some(format!(
                "请在此 Python 中安装 faster-whisper：{} -m pip install -U faster-whisper。错误：{}",
                resolve_python_path(python_path.as_deref()),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        }),
        Err(e) => Ok(DepCheckResult {
            name: "faster-whisper".into(),
            found: false,
            path: Some(py),
            version: None,
            suggestion: Some(format!("无法运行 Python 检测 faster-whisper：{e}")),
        }),
    }
}

/// 检测 GPU / CUDA
#[tauri::command]
pub async fn detect_gpu() -> Result<DepCheckResult, AppError> {
    let nvidia_smi = if cfg!(target_os = "windows") {
        "nvidia-smi.exe"
    } else {
        "nvidia-smi"
    };

    match Command::new(nvidia_smi)
        .arg("--query-gpu=name")
        .arg("--format=csv,noheader")
        .output()
    {
        Ok(output) if output.status.success() => {
            let gpu_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(DepCheckResult {
                name: "GPU".into(),
                found: true,
                path: Some(gpu_name),
                version: Some("CUDA 可用".into()),
                suggestion: None,
            })
        }
        _ => Ok(DepCheckResult {
            name: "GPU".into(),
            found: false,
            path: None,
            version: None,
            suggestion: Some(
                "未检测到 NVIDIA GPU。CPU 模式下 ASR 转写较慢，约为视频时长的 30-50%。".into(),
            ),
        }),
    }
}

/// 检测所有依赖（带可选的配置 Python 路径）
#[tauri::command]
pub async fn detect_all_deps(python_path: Option<String>) -> Result<AllDepResults, AppError> {
    let python = detect_python(python_path.clone()).await?;
    let ffmpeg = detect_ffmpeg().await?;
    let faster_whisper = detect_faster_whisper(python_path.clone()).await?;
    let ytdlp = detect_ytdlp(python_path).await?;
    let gpu = detect_gpu().await?;

    Ok(AllDepResults {
        python,
        ffmpeg,
        faster_whisper,
        ytdlp,
        gpu,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_python_version() {
        assert_eq!(parse_python_version("3.12.0"), Some((3, 12, 0)));
        assert_eq!(parse_python_version("3.9.1"), Some((3, 9, 1)));
        assert_eq!(parse_python_version("3.8.10"), Some((3, 8, 10)));
        assert_eq!(parse_python_version("2.7.18"), Some((2, 7, 18)));
        assert_eq!(parse_python_version("3.10"), Some((3, 10, 0)));
        assert_eq!(parse_python_version("invalid"), None);
        assert_eq!(parse_python_version(""), None);
    }

    #[test]
    fn test_version_comparison() {
        assert!((3, 12) >= PYTHON_MIN_VERSION);
        assert!((3, 9) >= PYTHON_MIN_VERSION);
        assert!(!((3, 8) >= PYTHON_MIN_VERSION));
        assert!(!((2, 7) >= PYTHON_MIN_VERSION));
    }
}
