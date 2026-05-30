// ============================================================
// Python 脚本调度 — 统一子进程调用模式
// 与 docs/architecture.md §2.3 对齐
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

/// Python 转写结果
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub model_size: String,
    pub device: String,
    pub compute_type: String,
    pub language: String,
    pub language_probability: f64,
    pub segment_count: usize,
    pub srt_path: PathBuf,
    pub text_path: PathBuf,
}

/// Python 关键帧提取结果
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeResult {
    pub frame_count: usize,
    pub output_dir: PathBuf,
    pub frames: Vec<KeyframeInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeInfo {
    pub filename: String,
    pub timestamp_seconds: f64,
    pub timestamp_label: String,
}

/// 通用 Python 脚本执行结果
#[derive(Debug, Serialize, Deserialize)]
pub struct PythonScriptResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// 执行 Python 脚本 — 通用模式
pub async fn run_python_script(
    python_path: &str,
    script_path: &PathBuf,
    args: &[&str],
) -> Result<PythonScriptResult, AppError> {
    let output = Command::new(python_path)
        .arg(script_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(AppError::Io)?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    let success = output.status.success();

    Ok(PythonScriptResult {
        success,
        stdout,
        stderr,
        exit_code,
    })
}

/// 执行音频转写 (faster-whisper)
#[tauri::command]
pub async fn transcribe_audio(
    audio_path: String,
    output_dir: String,
    python_path: String,
    model_size: String,
    device: String,
) -> Result<TranscriptionResult, AppError> {
    let result = run_python_script(
        &python_path,
        &PathBuf::from("scripts/transcribe_faster_whisper.py"),
        &[
            &audio_path,
            "--output-dir",
            &output_dir,
            "--model-size",
            &model_size,
            "--device",
            &device,
        ],
    )
    .await?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: "transcribe_faster_whisper".into(),
            stderr: result.stderr,
        });
    }

    let parsed: TranscriptionResult =
        serde_json::from_str(&result.stdout).map_err(|e| {
            AppError::Other(format!("解析转写结果 JSON 失败: {e}"))
        })?;

    Ok(parsed)
}

/// 执行关键帧提取
#[tauri::command]
pub async fn extract_keyframes(
    video_path: String,
    output_dir: String,
    python_path: String,
    interval: u32,
    max_frames: u32,
    mode: String,
) -> Result<KeyframeResult, AppError> {
    let result = run_python_script(
        &python_path,
        &PathBuf::from("scripts/extract_keyframes.py"),
        &[
            "--video",
            &video_path,
            "--output-dir",
            &output_dir,
            "--interval",
            &interval.to_string(),
            "--max-frames",
            &max_frames.to_string(),
            "--mode",
            &mode,
        ],
    )
    .await?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: "extract_keyframes".into(),
            stderr: result.stderr,
        });
    }

    let parsed: KeyframeResult =
        serde_json::from_str(&result.stdout).map_err(|e| {
            AppError::Other(format!("解析关键帧结果 JSON 失败: {e}"))
        })?;

    Ok(parsed)
}

/// 检测 Python 环境
#[tauri::command]
pub async fn check_python_env(python_path: String) -> Result<String, AppError> {
    let output = Command::new(&python_path)
        .args(["-c", "import sys; print(sys.version)"])
        .output()
        .await
        .map_err(AppError::Io)?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(AppError::MissingDependency(format!(
            "无法运行指定的 Python: {python_path}"
        )))
    }
}
