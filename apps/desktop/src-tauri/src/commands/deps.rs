// ============================================================
// 系统依赖检测 — Python / FFmpeg / yt-dlp / GPU
// 与 docs/architecture.md §2.4 对齐
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::process::Command;

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

/// 检测 Python 3.9+
#[tauri::command]
pub async fn detect_python() -> Result<DepCheckResult, AppError> {
    let candidates = if cfg!(target_os = "windows") {
        vec!["python", "python3", "py"]
    } else {
        vec!["python3", "python"]
    };

    for cmd_str in &candidates {
        if let Ok(output) = Command::new(cmd_str)
            .args(["--version"])
            .output()
        {
            if output.status.success() {
                let stdout =
                    String::from_utf8_lossy(&output.stdout).trim().to_string();
                let version = stdout
                    .strip_prefix("Python ")
                    .unwrap_or(&stdout)
                    .to_string();

                return Ok(DepCheckResult {
                    name: "Python".into(),
                    found: true,
                    path: Some(cmd_str.to_string()),
                    version: Some(version),
                    suggestion: None,
                });
            }
        }
    }

    Ok(DepCheckResult {
        name: "Python".into(),
        found: false,
        path: None,
        version: None,
        suggestion: Some(
            "请安装 Python 3.9+: https://www.python.org/downloads/".into(),
        ),
    })
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
            // 提取版本 (第一行 "ffmpeg version X.Y.Z...")
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
            suggestion: Some("pip install yt-dlp 或访问: https://github.com/yt-dlp/yt-dlp".into()),
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

    match Command::new(nvidia_smi).arg("--query-gpu=name").arg("--format=csv,noheader").output() {
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

/// 检测所有依赖
#[tauri::command]
pub async fn detect_all_deps() -> Result<AllDepResults, AppError> {
    let python = detect_python().await?;
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
