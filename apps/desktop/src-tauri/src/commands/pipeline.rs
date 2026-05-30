// ============================================================
// 管线编排 — 多步骤管线执行器
// 与 docs/architecture.md §2.2 对齐
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use std::path::PathBuf;

// ---- 数据结构 ----

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InputMode {
    #[serde(rename = "bilibili")] Bilibili,
    #[serde(rename = "youtube")] Youtube,
    #[serde(rename = "douyin")] Douyin,
    #[serde(rename = "xiaohongshu")] Xiaohongshu,
    #[serde(rename = "article_url")] ArticleUrl,
    #[serde(rename = "local_video")] LocalVideo,
    #[serde(rename = "local_audio")] LocalAudio,
    #[serde(rename = "local_text")] LocalText,
}

impl std::fmt::Display for InputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputMode::Bilibili => write!(f, "B 站视频"),
            InputMode::Youtube => write!(f, "YouTube 视频"),
            InputMode::Douyin => write!(f, "抖音/TikTok"),
            InputMode::Xiaohongshu => write!(f, "小红书"),
            InputMode::ArticleUrl => write!(f, "在线文章"),
            InputMode::LocalVideo => write!(f, "本地视频"),
            InputMode::LocalAudio => write!(f, "本地音频"),
            InputMode::LocalText => write!(f, "本地文档"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineProgressEvent {
    pub step: String,
    pub label: String,
    pub percent: f64,
    pub status: String, // "running" | "completed" | "failed"
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PipelineResult {
    pub success: bool,
    pub mode: String,
    pub note_path: Option<String>,
    pub error: Option<String>,
    pub duration_seconds: f64,
}

// ---- 执行入口 ----

/// 执行完整管线，通过 Tauri events 推送进度
#[tauri::command]
pub async fn execute_pipeline(
    app: AppHandle,
    input: String,
    mode: String,
    python_path: Option<String>,
    note_dir: String,
) -> Result<PipelineResult, AppError> {
    let start = std::time::Instant::now();
    let input_mode: InputMode =
        serde_json::from_str(&format!("\"{mode}\""))
            .unwrap_or(InputMode::ArticleUrl);

    emit_progress(&app, "start", "开始处理", 0.0, "running", Some(input.as_str()));

    // 步骤 1: 依赖检查
    emit_progress(&app, "deps", "环境检查", 5.0, "running", None);
    check_deps(&python_path)?;
    emit_progress(&app, "deps", "环境检查", 10.0, "completed", None);

    // 步骤 2-8: 根据模式执行
    match input_mode {
        InputMode::Bilibili | InputMode::Youtube | InputMode::Douyin |
        InputMode::Xiaohongshu | InputMode::LocalVideo => {
            run_video_pipeline(&app, &input, &input_mode, &python_path, &note_dir).await?;
        }
        InputMode::LocalAudio => {
            run_audio_pipeline(&app, &input, &python_path, &note_dir).await?;
        }
        InputMode::ArticleUrl | InputMode::LocalText => {
            run_text_pipeline(&app, &input, &note_dir).await?;
        }
    }

    // 步骤 9: 清理
    emit_progress(&app, "cleanup", "清理临时文件", 95.0, "running", None);
    // cleanup_temp_files() — 后续实现
    emit_progress(&app, "cleanup", "清理完成", 98.0, "completed", None);

    // 完成
    let duration = start.elapsed().as_secs_f64();
    let done_detail = format!("耗时 {:.1}s", duration);
    emit_progress(&app, "completed", "完成", 100.0, "completed", Some(&done_detail));

    Ok(PipelineResult {
        success: true,
        mode: input_mode.to_string(),
        note_path: None, // 后续填入实际路径
        error: None,
        duration_seconds: duration,
    })
}

// ---- 视频管线 ----

async fn run_video_pipeline(
    app: &AppHandle,
    input: &str,
    mode: &InputMode,
    python_path: &Option<String>,
    note_dir: &str,
) -> Result<(), AppError> {
    // 生成临时 ID
    let video_id = generate_temp_id(input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&video_id);
    std::fs::create_dir_all(&temp_dir).map_err(AppError::Io)?;

    let audio_path = temp_dir.join("audio.mp3");

    // 本地视频：直接提取音频；在线视频：先下载
    if matches!(mode, InputMode::Bilibili | InputMode::Youtube | InputMode::Douyin | InputMode::Xiaohongshu) {
        emit_progress(app, "download", "下载视频", 20.0, "running", None);
        // TODO: 实际下载视频（AI Douyin / yt-dlp）
        emit_progress(app, "download", "下载完成（跳过-未接入脚本）", 30.0, "completed", None);
    }

    // 提取音频
    emit_progress(app, "extract_audio", "提取音频", 35.0, "running", None);
    let video_path = temp_dir.join("video.mp4");
    if video_path.exists() {
        extract_audio_ffmpeg(&video_path, &audio_path)?;
    }
    emit_progress(app, "extract_audio", "音频提取完成", 45.0, "completed", None);

    // ASR 转写
    emit_progress(app, "transcribe", "语音转写", 50.0, "running", None);
    if audio_path.exists() {
        transcribe_with_python(python_path, &audio_path, &temp_dir)?;
    }
    emit_progress(app, "transcribe", "转写完成", 65.0, "completed", None);

    // 关键帧
    emit_progress(app, "keyframes", "提取关键帧", 70.0, "running", None);
    if video_path.exists() {
        extract_keyframes_with_python(python_path, &video_path, &temp_dir)?;
    }
    emit_progress(app, "keyframes", "关键帧提取完成", 75.0, "completed", None);

    // AI 笔记生成
    emit_progress(app, "generate_note", "AI 生成笔记", 80.0, "running", None);
    let text_path = temp_dir.join("text.txt");
    if text_path.exists() {
        let _text = std::fs::read_to_string(&text_path).unwrap_or_default();
        // TODO: 调用 Claude API 流式生成笔记
        // stream_note_generation(app, &text, note_dir).await?;
    }
    emit_progress(app, "generate_note", "笔记生成完成", 90.0, "completed",
        Some("（模拟）请接入 Claude API Key 以启用真实生成"));

    Ok(())
}

// ---- 音频管线 ----

async fn run_audio_pipeline(
    app: &AppHandle,
    input: &str,
    python_path: &Option<String>,
    note_dir: &str,
) -> Result<(), AppError> {
    let audio_id = generate_temp_id(input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&audio_id);
    std::fs::create_dir_all(&temp_dir).map_err(AppError::Io)?;

    emit_progress(app, "transcribe", "语音转写", 30.0, "running", None);
    let audio_path = PathBuf::from(input);
    transcribe_with_python(python_path, &audio_path, &temp_dir)?;
    emit_progress(app, "transcribe", "转写完成", 55.0, "completed", None);

    emit_progress(app, "generate_note", "AI 生成笔记", 60.0, "running", None);
    // TODO: Claude API
    emit_progress(app, "generate_note", "笔记生成完成", 90.0, "completed",
        Some("（模拟）请接入 Claude API Key 以启用真实生成"));

    Ok(())
}

// ---- 文本管线 (文章/文档) ----

async fn run_text_pipeline(
    app: &AppHandle,
    input: &str,
    note_dir: &str,
) -> Result<(), AppError> {
    emit_progress(app, "read", "读取内容", 15.0, "running", None);

    let text = if input.starts_with("http") {
        emit_progress(app, "read", "抓取网页内容", 25.0, "running", None);
        // TODO: WebFetch
        "（网页内容待抓取）".to_string()
    } else {
        std::fs::read_to_string(input).unwrap_or_else(|_| "（无法读取）".to_string())
    };

    let read_detail = format!("读取完成 ({}字)", text.len());
    emit_progress(app, "read", &read_detail, 40.0, "completed", None);

    emit_progress(app, "analyze", "AI 分析中", 50.0, "running", None);
    // TODO: Claude API summarize
    emit_progress(app, "analyze", "分析完成", 65.0, "completed", None);

    emit_progress(app, "generate_note", "生成笔记", 70.0, "running", None);
    // TODO: Claude API note generation
    emit_progress(app, "generate_note", "笔记生成完成", 90.0, "completed",
        Some("（模拟）请接入 Claude API Key 以启用真实生成"));

    Ok(())
}

// ---- 工具函数 ----

fn emit_progress(
    app: &AppHandle,
    step: &str,
    label: &str,
    percent: f64,
    status: &str,
    detail: Option<&str>,
) {
    let event = PipelineProgressEvent {
        step: step.to_string(),
        label: label.to_string(),
        percent,
        status: status.to_string(),
        detail: detail.map(|s| s.to_string()),
    };
    let _ = app.emit("pipeline-progress", event);
}

fn check_deps(python_path: &Option<String>) -> Result<(), AppError> {
    let py = python_path.as_deref().unwrap_or("python3");
    let output = std::process::Command::new(py)
        .args(["--version"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout);
            if v.contains("Python 3") {
                return Ok(());
            }
        }
        _ => {}
    }
    // 不阻塞：Python 可能在其他步骤才需要
    Ok(())
}

fn extract_audio_ffmpeg(video: &PathBuf, audio: &PathBuf) -> Result<(), AppError> {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-i",
            &video.to_string_lossy(),
            "-q:a", "0",
            "-map", "a",
            "-y",
            &audio.to_string_lossy(),
        ])
        .status()
        .map_err(|_| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;

    if !status.success() {
        return Err(AppError::Other("FFmpeg 音频提取失败".into()));
    }
    Ok(())
}

fn transcribe_with_python(
    python_path: &Option<String>,
    audio: &PathBuf,
    output_dir: &PathBuf,
) -> Result<(), AppError> {
    if !audio.exists() {
        return Ok(()); // skip silently
    }
    let py = python_path.as_deref().unwrap_or("python3");
    let script = PathBuf::from("scripts/transcribe_faster_whisper.py");
    if !script.exists() {
        return Ok(()); // script not yet added
    }

    let output = std::process::Command::new(py)
        .arg(&script)
        .arg(audio)
        .arg("--output-dir")
        .arg(output_dir)
        .output()
        .map_err(|_| AppError::MissingDependency(format!("无法运行 Python: {py}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::PythonScript {
            script: "transcribe_faster_whisper".into(),
            stderr: stderr.to_string(),
        });
    }
    Ok(())
}

fn extract_keyframes_with_python(
    python_path: &Option<String>,
    video: &PathBuf,
    output_dir: &PathBuf,
) -> Result<(), AppError> {
    if !video.exists() {
        return Ok(());
    }
    let py = python_path.as_deref().unwrap_or("python3");
    let script = PathBuf::from("scripts/extract_keyframes.py");
    if !script.exists() {
        return Ok(());
    }

    let _ = std::process::Command::new(py)
        .arg(&script)
        .arg("--video")
        .arg(video)
        .arg("--output-dir")
        .arg(output_dir)
        .output()
        .map_err(|_| AppError::MissingDependency(format!("无法运行 Python: {py}")))?;
    Ok(())
}

fn generate_temp_id(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.hash(&mut h);
    format!("{:x}", h.finish())
}
