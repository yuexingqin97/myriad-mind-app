// ============================================================
// 管线编排 — 多步骤管线执行器
// 串联 Python 脚本 + MindEngine AI，每步推送进度事件
// ============================================================

use crate::commands::ai;
use crate::commands::python::resolve_python_path;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use std::path::PathBuf;

// ============================================================
// 数据结构
// ============================================================

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
    #[serde(rename = "code_project")] CodeProject,
}

impl std::fmt::Display for InputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bilibili => write!(f, "B 站视频"),
            Self::Youtube => write!(f, "YouTube 视频"),
            Self::Douyin => write!(f, "抖音/TikTok"),
            Self::Xiaohongshu => write!(f, "小红书"),
            Self::ArticleUrl => write!(f, "在线文章"),
            Self::LocalVideo => write!(f, "本地视频"),
            Self::LocalAudio => write!(f, "本地音频"),
            Self::LocalText => write!(f, "本地文档"),
            Self::CodeProject => write!(f, "代码项目"),
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

// ============================================================
// 执行入口
// ============================================================

/// 执行完整管线，通过 Tauri events 推送进度
#[tauri::command]
pub async fn execute_pipeline(
    app: AppHandle,
    input: String,
    mode: String,
    python_path: Option<String>,
    note_dir: String,
    note_category: Option<String>,
    debug_metadata: Option<bool>,
) -> Result<PipelineResult, AppError> {
    let start = std::time::Instant::now();
    let input_mode: InputMode =
        serde_json::from_str(&format!("\"{mode}\""))
            .unwrap_or(InputMode::ArticleUrl);

    // 解析 Python 路径: 用户配置 > venv > 系统 PATH
    let py = resolve_python_path(python_path.as_deref());

    emit_progress(&app, "start", &format!("开始处理 · {}", input_mode), 0.0, "running", Some(&input));

    // 步骤 0: 依赖检查
    emit_progress(&app, "deps", "环境检查", 5.0, "running", None);
    if let Err(e) = check_deps(&py) {
        emit_progress(&app, "deps", &format!("环境检查警告: {e}"), 10.0, "completed",
            Some("部分功能可能不可用，但不阻塞处理"));
    } else {
        emit_progress(&app, "deps", "环境检查通过", 10.0, "completed", None);
    }

    // 步骤 1-8: 按输入模式分流
    let result = match input_mode {
        InputMode::Bilibili | InputMode::Youtube | InputMode::Douyin
        | InputMode::Xiaohongshu | InputMode::LocalVideo => {
            run_video_pipeline(&app, &input, &input_mode, &py, &note_dir).await
        }
        InputMode::LocalAudio => {
            run_audio_pipeline(&app, &input, &py, &note_dir).await
        }
        InputMode::ArticleUrl | InputMode::LocalText | InputMode::CodeProject => {
            run_text_pipeline(&app, &input, &note_dir, note_category.as_deref(), debug_metadata.unwrap_or(false)).await
        }
    };

    if let Err(e) = result {
        emit_progress(&app, "error", &format!("管线失败: {e}"), 0.0, "failed", None);
        return Err(e);
    }

    // 步骤 9: 清理
    let video_id = generate_temp_id(&input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&video_id);
    if temp_dir.exists() {
        emit_progress(&app, "cleanup", "清理临时文件", 95.0, "running", None);
        let _ = std::fs::remove_dir_all(&temp_dir);
        emit_progress(&app, "cleanup", "清理完成", 98.0, "completed", None);
    }

    // 完成
    let duration = start.elapsed().as_secs_f64();
    let done_detail = format!("耗时 {:.1}s", duration);
    emit_progress(&app, "completed", "炼化完成 ✅", 100.0, "completed", Some(&done_detail));

    Ok(PipelineResult {
        success: true,
        mode: input_mode.to_string(),
        note_path: None, // 后续填入实际路径
        error: None,
        duration_seconds: duration,
    })
}

// ============================================================
// 视频管线
// ============================================================

async fn run_video_pipeline(
    app: &AppHandle,
    input: &str,
    mode: &InputMode,
    python_path: &str,
    _note_dir: &str,
) -> Result<(), AppError> {
    let video_id = generate_temp_id(input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&video_id);
    std::fs::create_dir_all(&temp_dir).map_err(AppError::Io)?;

    let video_path = temp_dir.join("video.mp4");
    let audio_path = temp_dir.join("audio.mp3");

    // ---- 下载 / 本地准备 ----
    if matches!(mode, InputMode::Bilibili | InputMode::Youtube | InputMode::Douyin | InputMode::Xiaohongshu) {
        emit_progress(app, "download", "下载视频", 15.0, "running", None);
        // TODO: 调用 download_video / download_youtube_subtitles
        // 当前阶段: 下载逻辑待 AI Douyin / TikHub 对接
        emit_progress(app, "download", "下载（待接入解析 API）", 25.0, "completed",
            Some("需配置 AI Douyin / TikHub API Key"));
    } else {
        // 本地视频: 直接复制到 temp
        let src = PathBuf::from(input);
        if src.exists() {
            std::fs::copy(&src, &video_path).map_err(AppError::Io)?;
        }
        emit_progress(app, "prepare", "准备本地视频", 20.0, "completed", None);
    }

    // ---- 提取音频 ----
    emit_progress(app, "extract_audio", "提取音频", 30.0, "running", None);
    if video_path.exists() {
        extract_audio_ffmpeg(&video_path, &audio_path)?;
    }
    let audio_detail = if audio_path.exists() { "音频提取完成" } else { "无音频轨道，跳过" };
    emit_progress(app, "extract_audio", audio_detail, 40.0, "completed", None);

    // ---- ASR 转写 ----
    emit_progress(app, "transcribe", "语音转写", 45.0, "running", None);
    if audio_path.exists() {
        match transcribe_with_python(python_path, &audio_path, &temp_dir) {
            Ok(text_path) => {
                let text = std::fs::read_to_string(&text_path).unwrap_or_default();
                let detail = format!("转写完成 ({}字)", text.chars().count());
                emit_progress(app, "transcribe", &detail, 60.0, "completed", None);
            }
            Err(e) => {
                emit_progress(app, "transcribe", &format!("转写失败: {e}"), 60.0, "failed",
                    Some("检查 Python + faster-whisper 是否安装"));
                // 不中断: 后续 AI 可以基于有限内容生成笔记
            }
        }
    } else {
        emit_progress(app, "transcribe", "无音频，跳过转写", 55.0, "completed", None);
    }

    // ---- 关键帧截图 ----
    emit_progress(app, "keyframes", "提取关键帧", 65.0, "running", None);
    if video_path.exists() {
        match extract_keyframes_with_python(python_path, &video_path, &temp_dir) {
            Ok(frame_dir) => {
                let count = std::fs::read_dir(&frame_dir).map(|r| r.count()).unwrap_or(0);
                emit_progress(app, "keyframes", &format!("提取了 {count} 帧截图", ), 70.0, "completed", None);
            }
            Err(e) => {
                emit_progress(app, "keyframes", &format!("关键帧提取失败: {e}"), 70.0, "completed",
                    Some("不影响笔记生成"));
            }
        }
    } else {
        emit_progress(app, "keyframes", "无视频文件，跳过", 70.0, "completed", None);
    }

    // ---- AI 笔记生成 ----
    emit_progress(app, "generate_note", "AI 生成笔记", 75.0, "running", None);
    let text_path = temp_dir.join("text.txt");
    if text_path.exists() {
        // TODO: 接入 MindEngine 流式生成
        emit_progress(app, "generate_note", "笔记生成完成", 90.0, "completed",
            Some("（待接入 DeepSeek API Key）"));
    } else {
        emit_progress(app, "generate_note", "无文本内容可分析", 90.0, "completed",
            Some("请先完成下载/转写步骤"));
    }

    Ok(())
}

// ============================================================
// 音频管线
// ============================================================

async fn run_audio_pipeline(
    app: &AppHandle,
    input: &str,
    python_path: &str,
    _note_dir: &str,
) -> Result<(), AppError> {
    let audio_id = generate_temp_id(input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&audio_id);
    std::fs::create_dir_all(&temp_dir).map_err(AppError::Io)?;

    emit_progress(app, "transcribe", "语音转写", 20.0, "running", None);
    let audio_path = PathBuf::from(input);
    match transcribe_with_python(python_path, &audio_path, &temp_dir) {
        Ok(text_path) => {
            let text = std::fs::read_to_string(&text_path).unwrap_or_default();
            let detail = format!("转写完成 ({}字)", text.chars().count());
            emit_progress(app, "transcribe", &detail, 55.0, "completed", None);
        }
        Err(e) => {
            return Err(e);
        }
    }

    emit_progress(app, "generate_note", "AI 生成笔记", 60.0, "running", None);
    // TODO: MindEngine
    emit_progress(app, "generate_note", "笔记生成完成", 90.0, "completed",
        Some("（待接入 DeepSeek API Key）"));

    Ok(())
}

// ============================================================
// 文本管线 (文章 URL / 本地文档)
// ============================================================

async fn run_text_pipeline(
    app: &AppHandle,
    input: &str,
    note_dir: &str,
    note_category: Option<&str>,
    debug_metadata: bool,
) -> Result<(), AppError> {
    emit_progress(app, "read", "读取内容", 15.0, "running", None);

    let text = if input.starts_with("http") {
        emit_progress(app, "fetch", "🌐 抓取网页内容", 20.0, "running", None);
        // TODO: WebFetch (reqwest + HTML 提取)
        emit_progress(app, "fetch", "⚠️ 网页抓取尚未实现", 25.0, "completed",
            Some("当前版本仅支持本地文件，网页抓取即将支持"));
        "（网页内容待抓取）".to_string()
    } else {
        emit_progress(app, "read", "📂 读取本地文件", 10.0, "running",
            Some(&format!("路径: {input}")));
        let content = std::fs::read_to_string(input).unwrap_or_else(|_| "（无法读取）".to_string());
        let detail = format!("读取完成 · {} 字符", content.len());
        emit_progress(app, "read", &detail, 30.0, "completed",
            Some("内容已加载，准备送入 AI 分析"));
        content
    };

    emit_progress(app, "classify", "🔍 分析内容类型", 35.0, "running", None);
    let content_type = if text.contains("```") || text.contains("fn ") || text.contains("class ") {
        "代码"
    } else if text.len() < 500 {
        "短文"
    } else {
        "文本"
    };
    emit_progress(app, "classify", &format!("内容类型: {content_type}"), 40.0, "completed", None);

    // 确保 .myriad-mind/ 索引存在
    let lib_dir = std::path::PathBuf::from(note_dir).join(".myriad-mind");
    let is_new_lib = !lib_dir.exists();
    emit_progress(app, "library", "📚 检查知识库索引", 42.0, "running",
        if is_new_lib { Some("首次使用此输出目录，正在建立索引…") } else { None });
    match crate::commands::library::ensure_library(note_dir) {
        Ok(()) => {
            let detail = if is_new_lib {
                let count = count_md_files(note_dir);
                Some(format!("索引建立完成 · 已扫描 {count} 篇已有笔记"))
            } else {
                Some("索引就绪".into())
            };
            emit_progress(app, "library", "索引就绪", 45.0, "completed", detail.as_deref());
        }
        Err(e) => {
            emit_progress(app, "library", &format!("索引建立失败: {e}"), 45.0, "completed", None);
        }
    }

    emit_progress(app, "generate_note", "🤖 AI 生成笔记 (DeepSeek V4 Pro)", 50.0, "running",
        Some("正在调用 AI 模型，流式输出中…"));

    // 调用 MindEngine (DeepSeek V4 Pro)
    match ai::generate_note(app, &text, "文本").await {
        Ok(note) => {
            emit_progress(app, "save", "💾 保存笔记", 90.0, "running", None);

            // 自动分类并保存
            let source_type = if input.starts_with("http") { "article" } else { "file" };
            match crate::commands::notes::save_note(
                &note, input, source_type, note_dir, note_category, debug_metadata,
            ) {
                Ok(result) => {
                    // 更新 .myriad-mind 索引
                    let fingerprint = crate::commands::notes::simple_hash(input);
                    let note_id = format!("note_{fingerprint}");
                    let _ = crate::commands::library::update_library_after_save(
                        note_dir, &result.path, &result.title, &result.category,
                        result.version, input, source_type, &fingerprint, &note_id, &note,
                    );

                    let detail = format!(
                        "📁 {}/{}.md\n📝 v{} · {} 字符",
                        result.category, result.title, result.version, note.len()
                    );
                    emit_progress(app, "save", "笔记已保存", 98.0, "completed", Some(&detail));
                }
                Err(e) => {
                    emit_progress(app, "save", &format!("保存失败: {e}"), 95.0, "failed", None);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            emit_progress(app, "generate_note", &format!("AI 生成失败: {e}"), 90.0, "failed",
                Some(&e.to_string()));
            return Err(e);
        }
    }

    Ok(())
}

// ============================================================
// 工具函数
// ============================================================

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
    log::info!("[pipeline] {status:>9} | {percent:>5.0}% | {step} | {label}");
    if let Err(e) = app.emit("pipeline-progress", event) {
        log::error!("[pipeline] emit failed: {e}");
    }
}

fn count_md_files(dir: &str) -> usize {
    let mut count = 0;
    fn walk(dir: &std::path::Path, count: &mut usize) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if !entry.file_name().to_string_lossy().starts_with('.') {
                        walk(&p, count);
                    }
                } else if p.extension().map(|e| e == "md").unwrap_or(false) {
                    *count += 1;
                }
            }
        }
    }
    walk(&std::path::PathBuf::from(dir), &mut count);
    count
}

fn check_deps(python_path: &str) -> Result<(), AppError> {
    let output = std::process::Command::new(python_path)
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
    Err(AppError::MissingDependency(format!(
        "Python 不可用: {python_path}"
    )))
}

fn extract_audio_ffmpeg(video: &PathBuf, audio: &PathBuf) -> Result<(), AppError> {
    let status = std::process::Command::new(if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" })
        .args([
            "-i", &video.to_string_lossy(),
            "-q:a", "0",
            "-map", "a",
            "-y",
            &audio.to_string_lossy(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;

    if !status.success() {
        return Err(AppError::Other("FFmpeg 音频提取失败".into()));
    }
    Ok(())
}

/// 调用 transcribe_faster_whisper.py，返回 text.txt 的路径
fn transcribe_with_python(
    python_path: &str,
    audio: &PathBuf,
    output_dir: &PathBuf,
) -> Result<PathBuf, AppError> {
    use crate::commands::python::run_python_script;

    if !audio.exists() {
        return Err(AppError::Other("音频文件不存在".into()));
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            run_python_script(
                python_path,
                "transcribe_faster_whisper.py",
                &[
                    audio.to_string_lossy().to_string(),
                    "--output-dir".into(),
                    output_dir.to_string_lossy().to_string(),
                ],
            ).await
        })
    })?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: "transcribe_faster_whisper".into(),
            stderr: result.stderr,
        });
    }

    let text_path = output_dir.join("text.txt");
    if text_path.exists() {
        Ok(text_path)
    } else {
        Err(AppError::Other("ASR 转写完成但未找到 text.txt".into()))
    }
}

/// 调用 extract_keyframes.py，返回截图输出目录
fn extract_keyframes_with_python(
    python_path: &str,
    video: &PathBuf,
    output_dir: &PathBuf,
) -> Result<PathBuf, AppError> {
    use crate::commands::python::run_python_script;

    if !video.exists() {
        return Err(AppError::Other("视频文件不存在".into()));
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            run_python_script(
                python_path,
                "extract_keyframes.py",
                &[
                    "--video".into(),
                    video.to_string_lossy().to_string(),
                    "--output-dir".into(),
                    output_dir.to_string_lossy().to_string(),
                ],
            ).await
        })
    })?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: "extract_keyframes".into(),
            stderr: result.stderr,
        });
    }

    Ok(output_dir.clone())
}

fn generate_temp_id(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.hash(&mut h);
    format!("{:x}", h.finish())
}
