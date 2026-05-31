// ============================================================
// 系统依赖检测 — Python / FFmpeg / yt-dlp / GPU
// 优先使用配置中的 python_path，再回退 PATH 探测
// Python 版本校验 >= 3.9
// ============================================================

use crate::commands::python::resolve_python_path;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
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
pub async fn detect_python(
    python_path: Option<String>,
) -> Result<DepCheckResult, AppError> {
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
    let output = match Command::new(python_cmd).args(["--version"]).output() {
        Ok(o) if o.status.success() => o,
        _ => {
            return Ok(DepCheckResult {
                name: "Python".into(),
                found: false,
                path: None,
                version: None,
                suggestion: Some(
                    "请安装 Python 3.9+: https://www.python.org/downloads/".into(),
                ),
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
        Some((major, minor, _)) if (major, minor) >= PYTHON_MIN_VERSION => {
            Ok(DepCheckResult {
                name: "Python".into(),
                found: true,
                path: Some(python_cmd.to_string()),
                version: Some(version_str),
                suggestion: None,
            })
        }
        Some((major, minor, _)) => {
            Ok(DepCheckResult {
                name: "Python".into(),
                found: false,
                path: Some(python_cmd.to_string()),
                version: Some(version_str),
                suggestion: Some(format!(
                    "Python 版本过低: {}.{}，需要 >= {}.{}",
                    major, minor,
                    PYTHON_MIN_VERSION.0, PYTHON_MIN_VERSION.1
                )),
            })
        }
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
    let cmd = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };

    match Command::new(cmd).args(["-version"]).output() {
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
            suggestion: Some("请安装 FFmpeg: https://ffmpeg.org/download.html".into()),
        }),
    }
}

/// 检测 yt-dlp
#[tauri::command]
pub async fn detect_ytdlp() -> Result<DepCheckResult, AppError> {
    let cmd = if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };

    match Command::new(cmd).args(["--version"]).output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            Ok(DepCheckResult {
                name: "yt-dlp".into(),
                found: true,
                path: Some(cmd.to_string()),
                version: Some(version),
                suggestion: None,
            })
        }
        _ => Ok(DepCheckResult {
            name: "yt-dlp".into(),
            found: false,
            path: None,
            version: None,
            suggestion: Some(
                "pip install yt-dlp 或访问: https://github.com/yt-dlp/yt-dlp".into(),
            ),
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
            let gpu_name = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
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
pub async fn detect_all_deps(
    python_path: Option<String>,
) -> Result<AllDepResults, AppError> {
    let python = detect_python(python_path).await?;
    let ffmpeg = detect_ffmpeg().await?;
    let ytdlp = detect_ytdlp().await?;
    let gpu = detect_gpu().await?;

    Ok(AllDepResults {
        python,
        ffmpeg,
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
