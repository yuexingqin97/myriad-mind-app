// ============================================================
// 管线编排 — 多步骤管线执行器
// 串联 Python 脚本 + MindEngine AI，每步推送进度事件
// ============================================================

use crate::commands::ai;
use crate::commands::python::{python_command_parts, resolve_python_path};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

const PYTHON_MIN_VERSION: (u32, u32) = (3, 9);

// ============================================================
// 数据结构
// ============================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InputMode {
    #[serde(rename = "bilibili")]
    Bilibili,
    #[serde(rename = "youtube")]
    Youtube,
    #[serde(rename = "douyin")]
    Douyin,
    #[serde(rename = "xiaohongshu")]
    Xiaohongshu,
    #[serde(rename = "article_url")]
    ArticleUrl,
    #[serde(rename = "local_video")]
    LocalVideo,
    #[serde(rename = "local_audio")]
    LocalAudio,
    #[serde(rename = "local_text")]
    LocalText,
    #[serde(rename = "code_project")]
    CodeProject,
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
    task_prompt: Option<String>,
    debug_metadata: Option<bool>,
    cleanup_temp: Option<bool>,
) -> Result<PipelineResult, AppError> {
    let start = std::time::Instant::now();
    let input_mode: InputMode =
        serde_json::from_str(&format!("\"{mode}\"")).unwrap_or(InputMode::ArticleUrl);

    // 解析 Python 路径: 用户配置 > venv > 系统 PATH
    let py = resolve_python_path(python_path.as_deref());

    emit_progress(
        &app,
        "start",
        &format!("开始处理 · {}", input_mode),
        0.0,
        "running",
        Some(&input),
    );

    // 步骤 0: 依赖检查
    emit_progress(&app, "deps", "环境检查", 5.0, "running", None);
    if let Err(e) = validate_pipeline_deps(&input_mode, &py) {
        // 缺失依赖的英文原因（FFmpeg/yt-dlp 未找到等）走开发者日志
        log::warn!(
            target: "agent",
            "[pipeline] phase=deps status=failed err={e}"
        );
        emit_progress(
            &app,
            "deps",
            "环境检查未通过",
            8.0,
            "failed",
            Some("请到设置页修复依赖后重新检测"),
        );
        return Err(e);
    } else {
        emit_progress(&app, "deps", "环境检查通过", 10.0, "completed", None);
    }

    // 步骤 1-8: 按输入模式分流
    let result = match input_mode {
        InputMode::Bilibili
        | InputMode::Youtube
        | InputMode::Douyin
        | InputMode::Xiaohongshu
        | InputMode::LocalVideo => {
            run_video_pipeline(
                &app,
                &input,
                &input_mode,
                &py,
                &note_dir,
                note_category.as_deref(),
                task_prompt.as_deref(),
                debug_metadata.unwrap_or(false),
            )
            .await
        }
        InputMode::LocalAudio => run_audio_pipeline(&app, &input, &py, &note_dir).await,
        InputMode::ArticleUrl | InputMode::LocalText | InputMode::CodeProject => {
            run_text_pipeline(
                &app,
                &input,
                &input_mode,
                &note_dir,
                note_category.as_deref(),
                task_prompt.as_deref(),
                debug_metadata.unwrap_or(false),
            )
            .await
        }
    };

    if let Err(e) = result {
        emit_progress(
            &app,
            "error",
            &format!("管线失败: {e}"),
            0.0,
            "failed",
            None,
        );
        return Err(e);
    }

    // 步骤 9: 清理
    let should_cleanup = cleanup_temp.unwrap_or(true);
    let video_id = generate_temp_id(&input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&video_id);
    if should_cleanup && temp_dir.exists() {
        emit_progress(&app, "cleanup", "清理临时文件", 95.0, "running", None);
        let _ = std::fs::remove_dir_all(&temp_dir);
        emit_progress(&app, "cleanup", "清理完成", 98.0, "completed", None);
    } else if !should_cleanup && temp_dir.exists() {
        emit_progress(
            &app,
            "cleanup",
            "保留临时文件（按设置跳过清理）",
            98.0,
            "completed",
            Some(&format!("临时目录: {}", temp_dir.display())),
        );
    }

    // 完成
    let duration = start.elapsed().as_secs_f64();
    let done_detail = format!("耗时 {:.1}s", duration);
    emit_progress(
        &app,
        "completed",
        "炼化完成 ✅",
        100.0,
        "completed",
        Some(&done_detail),
    );

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
    note_dir: &str,
    note_category: Option<&str>,
    task_prompt: Option<&str>,
    debug_metadata: bool,
) -> Result<(), AppError> {
    let video_id = generate_temp_id(input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&video_id);
    std::fs::create_dir_all(&temp_dir).map_err(AppError::Io)?;

    let video_path = temp_dir.join("video.mp4");
    let audio_path = temp_dir.join("audio.mp3");
    let subtitle_path = temp_dir.join("subtitle.srt");

    // ---- 元信息预取（yt-dlp --dump-json，不下载） ----
    let mut video_title = String::new();
    let mut video_author = String::new();
    let mut video_duration = 0.0_f64;

    if is_online_video(mode) {
        emit_progress(app, "metadata", "📋 获取视频信息", 8.0, "running", None);
        if let Some((title, author, duration)) = fetch_video_metadata(python_path, input) {
            video_title = title;
            video_author = author;
            video_duration = duration;
            let dur_min = (duration / 60.0) as u32;
            emit_progress(
                app,
                "metadata",
                &format!("{video_title} · {video_author} · ~{dur_min}分钟"),
                12.0,
                "completed",
                None,
            );
        } else {
            emit_progress(app, "metadata", "元信息获取跳过", 12.0, "completed", None);
        }
    }

    let mut text_content = String::new();

    // YouTube: match the skill flow by trying subtitles before downloading media.
    if matches!(mode, InputMode::Youtube) {
        emit_progress(
            app,
            "subtitles",
            "📝 优先抓取 YouTube 字幕",
            10.0,
            "running",
            Some("先尝试人工字幕/自动字幕，失败后再回退下载音频 + ASR"),
        );
        match download_youtube_subtitles_with_python(python_path, input, &temp_dir) {
            Ok(text_path) => {
                text_content = std::fs::read_to_string(&text_path).unwrap_or_default();
                let chars = text_content.chars().count();
                emit_progress(
                    app,
                    "subtitles",
                    &format!("字幕抓取完成 · {} 字符", chars),
                    18.0,
                    "completed",
                    Some("已跳过后续 ASR 转写需求"),
                );
            }
            Err(e) => {
                emit_progress(
                    app,
                    "subtitles",
                    &format!("字幕不可用，回退 ASR: {e}"),
                    18.0,
                    "completed",
                    Some("将下载视频并提取音频"),
                );
            }
        }
    }

    // ---- 下载 / 本地准备 ----
    if matches!(
        mode,
        InputMode::Bilibili | InputMode::Youtube | InputMode::Douyin | InputMode::Xiaohongshu
    ) {
        let is_bilibili = matches!(mode, InputMode::Bilibili);
    let is_douyin_or_xhs = matches!(mode, InputMode::Douyin | InputMode::Xiaohongshu);
    let optional_download = matches!(mode, InputMode::Youtube) && !text_content.is_empty();
    emit_progress(
        app,
        "download",
        "📥 下载视频",
        18.0,
        "running",
        Some(&format!("平台: {mode}")),
    );

    let download_result = if media_file_ready(&video_path) {
        // 缓存命中：video.mp4 已存在（上次 cleanup_temp=false 保留），跳过下载省 AI Douyin 积分
        emit_progress(
            app,
            "download",
            "♻️ 使用缓存视频（跳过下载，省 AI Douyin 积分）",
            18.0,
            "completed",
            Some("命中 temp 缓存。需重新下载：设置关闭「保留临时文件」开关"),
        );
        log::debug!(target: "agent",
            "[pipeline] cache hit: {} exists, skip download",
            video_path.display()
        );
        Ok::<String, AppError>(video_title.clone())
    } else if is_douyin_or_xhs {
        // 抖音 / 小红书 → 必须走 AI Douyin API
        download_douyin_video(python_path, input, &video_path, &temp_dir).await
            .map_err(|e| {
                log::warn!("[pipeline] douyin download failed: {e}");
                e
            })
    } else if is_bilibili {
        // B站 → 优先 AI Douyin，Key 没配或失败则回退 yt-dlp
        match download_douyin_video(python_path, input, &video_path, &temp_dir).await {
            Ok(title) => Ok(title),
            Err(e) => {
                // 原始错误（可能含上游 HTTP body / API 错误原文）走开发者日志，
                // detail 只给用户一句中文说明，避免技术细节进 LogPanel。
                log::error!(
                    target: "agent",
                    "[pipeline] phase=download status=ai_douyin_failed fallback=ytdlp err={e}"
                );
                emit_progress(
                    app, "download",
                    "⚠️ AI Douyin 失败，回退 yt-dlp",
                    18.0, "running",
                    Some("AI Douyin 解析失败，改用 yt-dlp 直接下载"),
                );
                download_video_ytdlp(python_path, input, mode, &video_path)
            }
        }
    } else {
        // YouTube → yt-dlp
        download_video_ytdlp(python_path, input, mode, &video_path)
    };

        match download_result {
            Ok(title) => {
                if video_title.is_empty() {
                    video_title = title;
                }
                emit_progress(
                    app,
                    "download",
                    &format!("下载完成: {video_title}"),
                    22.0,
                    "completed",
                    Some(&format!("文件: {}", video_path.display())),
                );
            }
            Err(e) => {
                log::warn!(
                    target: "agent",
                    "[pipeline] phase=download status=failed optional={optional_download} err={e}"
                );
                if optional_download {
                    emit_progress(
                        app,
                        "download",
                        "视频下载跳过",
                        22.0,
                        "completed",
                        Some("已获得字幕文本，将继续生成笔记；只是没有关键帧截图"),
                    );
                } else {
                    emit_progress(
                        app,
                        "download",
                        &format!("下载失败: {e}"),
                        22.0,
                        "failed",
                        Some("请检查 yt-dlp 是否安装、网络是否正常"),
                    );
                    return Err(e);
                }
            }
        }
    } else {
        // 本地视频: 直接复制到 temp
        let src = PathBuf::from(input);
        if src.exists() {
            std::fs::copy(&src, &video_path).map_err(AppError::Io)?;
            video_title = src
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
        }
        emit_progress(app, "prepare", "准备本地视频", 18.0, "completed", None);
    }

    // ---- 提取音频 ----
    emit_progress(app, "extract_audio", "🎵 提取音频", 24.0, "running", None);
    if !text_content.is_empty() {
        emit_progress(
            app,
            "extract_audio",
            "已有字幕文本，跳过音频提取",
            30.0,
            "completed",
            None,
        );
    } else if media_file_ready(&video_path) {
        match extract_audio_ffmpeg(&video_path, &audio_path) {
            Ok(()) if media_file_ready(&audio_path) => {
                emit_progress(
                    app,
                    "extract_audio",
                    "音频提取完成",
                    30.0,
                    "completed",
                    None,
                );
            }
            Ok(()) => {
                emit_progress(
                    app,
                    "extract_audio",
                    "音频提取为空，尝试直接下载音频",
                    28.0,
                    "running",
                    None,
                );
                if is_online_video(mode) {
                    match download_audio_ytdlp(python_path, input, &audio_path) {
                        Ok(()) if media_file_ready(&audio_path) => emit_progress(
                            app,
                            "extract_audio",
                            "音频下载完成",
                            30.0,
                            "completed",
                            Some(&format!("文件: {}", audio_path.display())),
                        ),
                        Ok(()) | Err(_) => {
                            emit_progress(
                                app,
                                "extract_audio",
                                "音频提取失败：未生成有效音频文件",
                                30.0,
                                "failed",
                                Some("请确认视频本身包含音轨，或检查 yt-dlp/FFmpeg 输出"),
                            );
                        }
                    }
                }
            }
            Err(e) => {
                // 原始错误（含 FFmpeg 英文 stderr / AppError 堆栈）走开发者日志，
                // 不进 emit_progress.detail —— detail 经 usePipeline.ts 直接渲染进用户 LogPanel。
                log::warn!(
                    target: "agent",
                    "[pipeline] phase=extract_audio status=ffmpeg_failed err={e}"
                );
                if is_online_video(mode) {
                    emit_progress(
                        app,
                        "extract_audio",
                        "视频抽音频失败，尝试直接下载音频",
                        28.0,
                        "running",
                        Some("FFmpeg 抽取失败，改为用 yt-dlp 下载音轨"),
                    );
                    match download_audio_ytdlp(python_path, input, &audio_path) {
                        Ok(()) if media_file_ready(&audio_path) => emit_progress(
                            app,
                            "extract_audio",
                            "音频下载完成",
                            30.0,
                            "completed",
                            Some(&format!("文件: {}", audio_path.display())),
                        ),
                        Ok(()) => emit_progress(
                            app,
                            "extract_audio",
                            "音频下载完成但文件为空",
                            30.0,
                            "failed",
                            Some("请检查源视频是否有可下载音轨"),
                        ),
                        Err(download_err) => {
                            log::warn!(
                                target: "agent",
                                "[pipeline] phase=extract_audio status=download_fallback_failed err={download_err}"
                            );
                            emit_progress(
                                app,
                                "extract_audio",
                                "音频提取失败",
                                30.0,
                                "failed",
                                Some("FFmpeg 抽取失败，yt-dlp 直接下载音频也失败"),
                            )
                        }
                    }
                } else {
                    emit_progress(
                        app,
                        "extract_audio",
                        "音频提取失败",
                        30.0,
                        "failed",
                        Some("请确认视频包含音轨，或检查 FFmpeg 是否安装"),
                    );
                }
            }
        }
    } else if is_online_video(mode) {
        emit_progress(
            app,
            "extract_audio",
            "未找到视频文件，尝试直接下载音频",
            28.0,
            "running",
            None,
        );
        match download_audio_ytdlp(python_path, input, &audio_path) {
            Ok(()) if media_file_ready(&audio_path) => emit_progress(
                app,
                "extract_audio",
                "音频下载完成",
                30.0,
                "completed",
                Some(&format!("文件: {}", audio_path.display())),
            ),
            Ok(()) => emit_progress(
                app,
                "extract_audio",
                "音频下载完成但文件为空",
                30.0,
                "failed",
                Some("请检查源视频是否有可下载音轨"),
            ),
            Err(e) => {
                log::warn!(
                    target: "agent",
                    "[pipeline] phase=extract_audio status=download_failed err={e}"
                );
                emit_progress(
                    app,
                    "extract_audio",
                    "音频下载失败",
                    30.0,
                    "failed",
                    Some("没有字幕文本，也没有可用音频，无法继续 ASR"),
                )
            }
        }
    }

    // ---- ASR 转写 ----
    emit_progress(
        app,
        "transcribe",
        "🎙️ 语音转写 (faster-whisper)",
        32.0,
        "running",
        Some("可能需要几分钟，取决于音频长度…"),
    );
    if !text_content.is_empty() {
        emit_progress(
            app,
            "transcribe",
            "已有字幕文本，跳过 ASR",
            45.0,
            "completed",
            None,
        );
    } else if media_file_ready(&audio_path) {
        match transcribe_with_python(python_path, &audio_path, &temp_dir) {
            Ok(text_path) => {
                text_content = std::fs::read_to_string(&text_path).unwrap_or_default();
                let chars = text_content.chars().count();
                emit_progress(
                    app,
                    "transcribe",
                    &format!("转写完成 · {} 字符", chars),
                    45.0,
                    "completed",
                    None,
                );
            }
            Err(e) => {
                emit_progress(
                    app,
                    "transcribe",
                    &format!("转写失败: {e}"),
                    45.0,
                    "failed",
                    Some("请检查 Python 环境: pip install faster-whisper"),
                );
            }
        }
    } else {
        emit_progress(
            app,
            "transcribe",
            "没有可用音频，无法转写",
            40.0,
            "failed",
            Some("请查看上一条“提取音频/下载音频”的失败原因"),
        );
        return Err(AppError::Other("没有可用音频，无法转写".into()));
    }

    // ---- 步骤 4.5: AI 字幕分析 → 推荐截图时间点 ----
    let guided_timestamps_path = temp_dir.join("guided_timestamps.json");
    // 优先用 yt-dlp 元信息时长，回退 ffprobe
    let effective_duration = if video_duration > 0.0 {
        video_duration
    } else {
        get_video_duration(&video_path)
    };
    if subtitle_path.exists() && video_path.exists() {
        emit_progress(
            app,
            "subtitle_analysis",
            "🧠 AI 分析字幕 → 推荐截图时间点",
            47.0,
            "running",
            Some("DeepSeek 正在识别画面价值高的时刻…"),
        );
        match std::fs::read_to_string(&subtitle_path) {
            Ok(srt_content) => {
                match ai::vision::analyze_subtitle(&srt_content, effective_duration, None).await {
                    Ok(timestamps) => {
                        if !timestamps.is_empty() {
                            if let Ok(json) = serde_json::to_string_pretty(&timestamps) {
                                let _ = std::fs::write(&guided_timestamps_path, &json);
                                emit_progress(
                                    app,
                                    "subtitle_analysis",
                                    &format!("字幕分析完成 · {} 个推荐时间点", timestamps.len()),
                                    50.0,
                                    "completed",
                                    Some(&format!("已保存到 {}", guided_timestamps_path.display())),
                                );
                            }
                        } else {
                            emit_progress(
                                app,
                                "subtitle_analysis",
                                "字幕分析：纯谈话内容，跳过截图引导",
                                50.0,
                                "completed",
                                None,
                            );
                        }
                    }
                    Err(e) => {
                        // 原始错误（DeepSeek 响应原文 / 解析堆栈）走开发者日志，
                        // detail 只给用户一句中文说明。
                        log::warn!(
                            target: "agent",
                            "[pipeline] phase=subtitle_analysis status=failed err={e}"
                        );
                        emit_progress(
                            app,
                            "subtitle_analysis",
                            "字幕分析跳过",
                            50.0,
                            "completed",
                            Some("不影响笔记生成，将使用常规截图方式"),
                        );
                    }
                }
            }
            Err(e) => {
                log::warn!("[pipeline] failed to read subtitle.srt: {e}");
            }
        }
    }

    // ---- 步骤 4.7: 关键帧截图 (支持引导时间点) ----
    emit_progress(app, "keyframes", "🖼️ 提取关键帧", 52.0, "running", None);
    let frames_dir = temp_dir.join("frames");
    if video_path.exists() {
        match extract_keyframes_guided(
            python_path,
            &video_path,
            &temp_dir,
            guided_timestamps_path
                .exists()
                .then_some(guided_timestamps_path.as_path()),
        ) {
            Ok(_frame_dir) => {
                emit_progress(app, "keyframes", "关键帧提取完成", 58.0, "completed", None);
            }
            Err(e) => {
                emit_progress(
                    app,
                    "keyframes",
                    &format!("关键帧跳过: {e}"),
                    58.0,
                    "completed",
                    Some("不影响笔记生成"),
                );
            }
        }
    }

    // ---- 步骤 7.1: AI 截图审查 + 步骤 7.4: 教程检测 ----
    let mut review_table = String::new();
    let mut is_tutorial = false;
    let keyframes_json_path = temp_dir.join("frames").join("keyframes.json");

    if frames_dir.exists() && keyframes_json_path.exists() {
        // 截图审查
        if let Ok(json_str) = std::fs::read_to_string(&keyframes_json_path) {
            if let Ok(keyframes_json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                emit_progress(
                    app,
                    "screenshot_review",
                    "🔍 AI 智能审查截图 (DeepSeek Vision)",
                    60.0,
                    "running",
                    Some("正在逐张分析截图价值…"),
                );

                let review_config = ai::types::ScreenshotReviewConfig::default();
                match ai::vision::review_keyframes(
                    &keyframes_json,
                    &text_content,
                    &frames_dir,
                    &review_config,
                )
                .await
                {
                    Ok(result) => {
                        review_table = result.review_table;
                        let detail = format!(
                            "共 {} 张 → 选中 {} 张嵌入笔记",
                            result.total,
                            result.selected.len()
                        );
                        emit_progress(app, "screenshot_review", &detail, 65.0, "completed", None);

                        // 复制选中截图到笔记 assets 目录
                        if !result.selected.is_empty() {
                            let assets_dir = PathBuf::from(note_dir).join("assets").join(&video_id);
                            let _ = std::fs::create_dir_all(&assets_dir);
                            for f in &result.selected {
                                let src = frames_dir.join(&f.file);
                                let dst = assets_dir.join(&f.file);
                                if src.exists() {
                                    let _ = std::fs::copy(&src, &dst);
                                }
                            }
                            log::debug!(target: "agent",
                                "[pipeline] copied {} screenshots to assets/{}",
                                result.selected.len(),
                                video_id
                            );
                        }
                    }
                    Err(e) => {
                        log::warn!("[pipeline] screenshot review failed: {e}");
                        emit_progress(
                            app,
                            "screenshot_review",
                            &format!("截图审查跳过: {e}"),
                            65.0,
                            "completed",
                            Some("所有截图将嵌入笔记"),
                        );
                    }
                }
            }
        }

        // 教程模式检测（取前 5 张截图）
        if !video_title.is_empty() {
            let first_frames: Vec<PathBuf> = if let Ok(entries) = std::fs::read_dir(&frames_dir) {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "png")
                            .unwrap_or(false)
                    })
                    .map(|e| e.path())
                    .take(5)
                    .collect()
            } else {
                vec![]
            };

            if !first_frames.is_empty() {
                match ai::vision::detect_tutorial_mode(&video_title, &first_frames).await {
                    Ok(result) => {
                        is_tutorial = result.is_tutorial;
                        if is_tutorial {
                            emit_progress(
                                app,
                                "tutorial_detect",
                                &format!(
                                    "📋 检测到操作型教程 (置信度 {:.0}%)",
                                    result.confidence * 100.0
                                ),
                                67.0,
                                "completed",
                                Some("将在笔记中生成操作流程图"),
                            );
                        }
                    }
                    Err(e) => {
                        log::warn!("[pipeline] tutorial detection failed: {e}");
                    }
                }
            }
        }
    }

    // ---- AI 笔记生成 ----
    let source_url = if is_online_video(mode) { input } else { "" };
    let platform_name = mode.to_string();

    let dur_display = if video_duration > 0.0 {
        format!(
            "{}分{}秒",
            (video_duration / 60.0) as u32,
            (video_duration as u32) % 60
        )
    } else {
        "未知".to_string()
    };

    let mut ai_input = format!(
        "## 视频信息\n\
        原始链接: {source_url}\n\
        来源平台: {platform_name}\n\
        视频标题: {video_title}\n\
        作者: {video_author}\n\
        时长: {dur_display}\n\
        截图素材目录: assets/{video_id}/\n\
        生成文件: 视频={video_file}; 音频={audio_file}; 字幕={subtitle_file}\n\
        \n\
        ---\n\n",
        video_file = if video_path.exists() {
            video_path.display().to_string()
        } else {
            "未生成".into()
        },
        audio_file = if audio_path.exists() {
            audio_path.display().to_string()
        } else {
            "未生成".into()
        },
        subtitle_file = if subtitle_path.exists() {
            subtitle_path.display().to_string()
        } else {
            "未生成".into()
        },
    );

    if text_content.is_empty() {
        ai_input.push_str("（未能提取音频文本，请基于标题生成简要笔记）");
    } else {
        ai_input.push_str(&format!("## 字幕文本\n\n{text_content}\n\n---\n"));
    }

    // 注入截图审查结果（仅 AI 审查通过的截图）
    if !review_table.is_empty() {
        ai_input.push_str(&format!(
            "\n## 截图审查结果\n\n以下截图已通过 AI 审查，请按审查表中标注的嵌入位置放置截图。\n\
             每张截图下方必须配可点击时间戳（用原始链接 {source_url} 拼接 ?t=秒数 或 &t=秒数）。\n\
             引用格式：`![截图说明](assets/{video_id}/frame_XXXX.png)`\n\
             时间戳格式：`> 📸 [截图于 M:SS]({source_url}?t=总秒数)`\n\n{review_table}\n",
            video_id = video_id,
            source_url = if source_url.is_empty() {
                "原始链接"
            } else {
                source_url
            },
            review_table = review_table,
        ));
    }

    // 注入教程模式 flag
    if is_tutorial {
        ai_input.push_str(&format!(
            "\n⚠️ 本视频被检测为**操作型教程**。请在笔记中额外生成：\n\
             1. 📋 **操作流程总览**（Mermaid flowchart，每步标注时间戳，用 `click` 语法链接到 {source_url}?t=秒数）\n\
             2. 每个操作步骤下方放置对应截图\n\
             3. 流程图节点格式：`STEP1[▶ 0:00<br/>操作描述]`\n\n"
        ));
    }

    emit_progress(
        app,
        "generate_note",
        "🤖 AI 生成笔记 (DeepSeek V4 Pro)",
        70.0,
        "running",
        Some("正在调用 AI 模型…"),
    );
    match ai::generate_note(app, &ai_input, "视频", Some(note_dir), task_prompt).await {
        Ok(note) => {
            emit_progress(app, "save", "💾 保存笔记", 90.0, "running", None);
            let source_type = match mode {
                InputMode::Bilibili => "bilibili",
                InputMode::Youtube => "youtube",
                InputMode::Douyin => "douyin",
                InputMode::Xiaohongshu => "xiaohongshu",
                InputMode::LocalVideo => "local_video",
                _ => "file",
            };
            match crate::commands::notes::save_note(
                &note,
                input,
                source_type,
                note_dir,
                note_category,
                debug_metadata,
                None,
            ) {
                Ok(result) => {
                    let fingerprint = crate::commands::notes::simple_hash(input);
                    let note_id = format!("note_{}", &fingerprint);
                    let _ = crate::commands::library::update_library_after_save(
                        note_dir,
                        &result.path,
                        &result.title,
                        &result.category,
                        result.version,
                        input,
                        source_type,
                        &fingerprint,
                        &note_id,
                        &note,
                    );
                    emit_progress(
                        app,
                        "completed",
                        &format!("炼化完成 ✅ · {} / v{}", result.title, result.version),
                        100.0,
                        "completed",
                        Some(&format!(
                            "📁 {}/{}.md · {} 字符",
                            result.category,
                            result.title,
                            note.len()
                        )),
                    );
                }
                Err(e) => {
                    log::warn!(
                        target: "agent",
                        "[pipeline] phase=save status=failed err={e}"
                    );
                    emit_progress(app, "save", "保存失败", 95.0, "failed", Some("笔记写入磁盘失败，详情见日志文件"));
                    return Err(e);
                }
            }
        }
        Err(e) => {
            emit_progress(
                app,
                "generate_note",
                &format!("AI 生成失败: {e}"),
                80.0,
                "failed",
                None,
            );
            return Err(e);
        }
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
    emit_progress(
        app,
        "generate_note",
        "笔记生成完成",
        90.0,
        "completed",
        Some("（待接入 DeepSeek API Key）"),
    );

    Ok(())
}

// ============================================================
// 文本管线 (文章 URL / 本地文档)
// ============================================================

async fn run_text_pipeline(
    app: &AppHandle,
    input: &str,
    input_mode: &InputMode,
    note_dir: &str,
    note_category: Option<&str>,
    task_prompt: Option<&str>,
    debug_metadata: bool,
) -> Result<(), AppError> {
    // ============================================================
    // Phase 1: 读取/抓取内容
    // ============================================================
    let (content_for_ai, content_type) = match input_mode {
        InputMode::CodeProject => {
            // 代码项目 → 目录扫描
            emit_progress(app, "scan", "📂 扫描代码项目", 15.0, "running", Some(input));
            let root = std::path::Path::new(input);
            let scan = crate::commands::code_project::scan_code_project(root, 3)?;
            let detail = format!(
                "{} 文件 · {} KB · {} · {}",
                scan.total_files,
                scan.total_size_bytes / 1024,
                scan.size_label,
                scan.tech_stack.join(", ")
            );
            emit_progress(app, "scan", &detail, 30.0, "completed", None);
            let ai_content = crate::commands::code_project::format_code_project_for_ai(&scan);
            (ai_content, "代码项目".to_string())
        }
        InputMode::ArticleUrl => {
            // 在线文章 → HTTP 抓取
            emit_progress(app, "fetch", "🌐 抓取网页内容", 15.0, "running", Some(input));
            match crate::commands::fetch::fetch_article(input).await {
                Ok(article) => {
                    let platform = &article.platform;
                    let lang = &article.language_hint;
                    let detail = format!(
                        "✅ 已抓取 · {} · {} · {} 字符",
                        platform,
                        lang,
                        article.body_text.len()
                    );
                    emit_progress(app, "fetch", &detail, 30.0, "completed", None);
                    let ai_content = crate::commands::fetch::format_article_for_ai(&article);
                    (ai_content, "网页文章".to_string())
                }
                Err(e) => {
                    emit_progress(
                        app,
                        "fetch",
                        &format!("❌ 抓取失败: {e}"),
                        25.0,
                        "failed",
                        Some("请尝试将页面另存为 HTML 文件，或直接粘贴文章内容"),
                    );
                    return Err(e);
                }
            }
        }
        InputMode::LocalText => {
            // 本地文件 → 直接读取
            emit_progress(
                app,
                "read",
                "📂 读取本地文件",
                10.0,
                "running",
                Some(&format!("路径: {input}")),
            );
            let content =
                std::fs::read_to_string(input).unwrap_or_else(|_| "（无法读取）".to_string());
            let detail = format!("读取完成 · {} 字符", content.len());
            emit_progress(
                app,
                "read",
                &detail,
                30.0,
                "completed",
                Some("内容已加载，准备送入 AI 分析"),
            );
            let ct = detect_content_type(&content);
            (content, ct.to_string())
        }
        _ => unreachable!(),
    };

    emit_progress(
        app,
        "classify",
        &format!("🔍 内容类型: {content_type}"),
        35.0,
        "completed",
        None,
    );

    // ============================================================
    // Phase 2: 知识库索引 + 指纹去重
    // ============================================================

    // 确保 .myriad-mind/ 索引存在
    let lib_dir = std::path::PathBuf::from(note_dir).join(".myriad-mind");
    let is_new_lib = !lib_dir.exists();
    emit_progress(
        app,
        "library",
        "📚 检查知识库索引",
        38.0,
        "running",
        if is_new_lib {
            Some("首次使用此输出目录，正在建立索引…")
        } else {
            None
        },
    );
    match crate::commands::library::ensure_library(note_dir) {
        Ok(()) => {
            let detail = if is_new_lib {
                let count = count_md_files(note_dir);
                Some(format!("索引建立完成 · 已扫描 {count} 篇已有笔记"))
            } else {
                Some("索引就绪".into())
            };
            emit_progress(app, "library", "索引就绪", 40.0, "completed", detail.as_deref());
        }
        Err(e) => {
            // 索引失败原因（文件系统错误等）走开发者日志，detail 只给用户中文短语
            log::warn!(
                target: "agent",
                "[pipeline] phase=library status=index_failed err={e}"
            );
            emit_progress(
                app,
                "library",
                "索引建立失败",
                40.0,
                "completed",
                Some("不影响本次笔记生成，详情见日志文件"),
            );
        }
    }

    // 检查指纹 — 是否命中已有笔记
    let fingerprint = crate::commands::notes::simple_hash(input);
    let existing_note = check_fingerprint_hit(note_dir, &fingerprint);
    let is_update = existing_note.is_some();

    if let Some(ref existing) = existing_note {
        emit_progress(
            app,
            "generate_note",
            &format!("🔁 命中已有笔记: {}", existing.path),
            42.0,
            "running",
            Some("检测到此输入曾炼化过，将基于已有笔记增量更新"),
        );
    }

    // ============================================================
    // Phase 3: AI 生成笔记
    // ============================================================

    emit_progress(
        app,
        "generate_note",
        "🤖 AI 生成笔记 (DeepSeek V4 Pro)",
        50.0,
        "running",
        Some(&format!("内容类型: {content_type}")),
    );

    // 准备增强上下文
    let enhanced_content = if let Some(ref existing) = existing_note {
        format!(
            "## 已有笔记（请在此基础上更新）\n\n{}\n\n---\n\n## 本次新增材料\n\n{}",
            existing.content, content_for_ai
        )
    } else {
        content_for_ai.clone()
    };

    // content_type 传给 generate_note 用于选择提示词分支
    let final_content_type = if is_update {
        // 更新模式：保留原 content_type 但标记为更新
        format!("更新·{}", content_type)
    } else {
        content_type.clone()
    };

    match ai::generate_note(
        app,
        &enhanced_content,
        &final_content_type,
        Some(note_dir),
        task_prompt,
    )
    .await
    {
        Ok(note) => {
            emit_progress(app, "save", "💾 保存笔记", 90.0, "running", None);

            // 自动分类并保存
            let source_type = if input.starts_with("http") {
                "article"
            } else if std::path::Path::new(input).is_dir() {
                "code_project"
            } else {
                "file"
            };
            match crate::commands::notes::save_note(
                &note,
                input,
                source_type,
                note_dir,
                note_category,
                debug_metadata,
                existing_note.as_ref().map(|n| n.path.as_str()),
            ) {
                Ok(result) => {
                    // 更新 .myriad-mind 索引
                    let fingerprint = crate::commands::notes::simple_hash(input);
                    let note_id = format!("note_{fingerprint}");
                    emit_progress(
                        app,
                        "library",
                        "📝 更新知识库索引",
                        96.0,
                        "running",
                        Some(&format!("分类: {} · v{}", result.category, result.version)),
                    );
                    let _ = crate::commands::library::update_library_after_save(
                        note_dir,
                        &result.path,
                        &result.title,
                        &result.category,
                        result.version,
                        input,
                        source_type,
                        &fingerprint,
                        &note_id,
                        &note,
                    );
                    emit_progress(app, "library", "索引已同步", 97.0, "completed", None);

                    let detail = format!(
                        "📁 {}/{}.md\n📝 v{} · {} 字符",
                        result.category,
                        result.title,
                        result.version,
                        note.len()
                    );
                    emit_progress(app, "save", "笔记已保存", 98.0, "completed", Some(&detail));
                }
                Err(e) => {
                    log::warn!(
                        target: "agent",
                        "[pipeline] phase=save status=failed err={e}"
                    );
                    emit_progress(app, "save", "保存失败", 95.0, "failed", Some("笔记写入磁盘失败，详情见日志文件"));
                    return Err(e);
                }
            }
        }
        Err(e) => {
            // AI 生成失败原因（DeepSeek 错误体等）走开发者日志，
            // label/detail 只给用户中文短语（detail 经 usePipeline 渲染进 LogPanel）
            log::error!(
                target: "agent",
                "[pipeline] phase=generate_note status=failed err={e}"
            );
            emit_progress(
                app,
                "generate_note",
                "AI 生成失败",
                90.0,
                "failed",
                Some("模型生成失败，请检查网络/API Key/日志文件后重试"),
            );
            return Err(e);
        }
    }

    Ok(())
}

// ============================================================
// 工具函数
// ============================================================

/// 内容类型检测 — 5 信号启发式（仅用于 LocalText 模式）
fn detect_content_type(text: &str) -> &'static str {
    let has_code_fences = text.contains("```");
    let has_fn = text.contains("fn ") || text.contains("function ");
    let has_class = text.contains("class ") || text.contains("impl ") || text.contains("struct ");
    let has_import = text.contains("import ") || text.contains("use ") || text.contains("#include");
    let has_pub = text.contains("pub fn") || text.contains("export ") || text.contains("def ");

    let code_signals = [has_code_fences, has_fn, has_class, has_import, has_pub]
        .iter()
        .filter(|&&x| x)
        .count();

    if code_signals >= 2 {
        "代码"
    } else if text.len() < 500 {
        "短文"
    } else {
        "文档"
    }
}

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
    // ---- 管线进度埋点（设计文档 §五「loop 每轮」）----
    // 结构化格式：`[pipeline] step=X status=Y percent=N`，label 作为附加上下文。
    // 用 debug 而非 info：emit_progress 高频触发，避免 release 默认 Info 级别刷屏
    // （用户日志走 pipeline-progress 事件 + UI LogPanel，不依赖此行）。
    log::debug!(
        target: "agent",
        "[pipeline] step={step} status={status} percent={percent:.0} label={label:?}"
    );
    if let Err(e) = app.emit("pipeline-progress", event) {
        log::error!("[pipeline] step={step} status={status} emit_failed err={e}");
    }
}

struct ExistingNote {
    path: String,
    content: String,
}

/// Check if this fingerprint already exists in the library
fn check_fingerprint_hit(base_dir: &str, fingerprint: &str) -> Option<ExistingNote> {
    let fp_path = std::path::PathBuf::from(base_dir)
        .join(".myriad-mind")
        .join("fingerprints.json");

    if let Ok(data) = std::fs::read_to_string(&fp_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            let fp_key = format!("sha256:{fingerprint}");
            if let Some(entry) = json["items"].get(&fp_key) {
                let path = entry["path"].as_str().unwrap_or("").to_string();
                let full_path = std::path::PathBuf::from(base_dir).join(&path);
                if full_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        log::debug!(target: "agent","[pipeline] fingerprint hit: {path}");
                        return Some(ExistingNote { path, content });
                    }
                }
            }
        }
    }
    None
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

/// 追问笔记：读取已有笔记 → AI 回答 → 追加到问答记录
#[allow(dead_code)]
#[tauri::command]
pub async fn execute_qa(
    app: AppHandle,
    note_path: String,
    question: String,
    write_back: bool,
) -> Result<String, AppError> {
    emit_progress(&app, "qa", "📖 读取笔记", 10.0, "running", Some(&note_path));

    let content = std::fs::read_to_string(&note_path)
        .map_err(|e| AppError::Config(format!("读取笔记失败: {e}")))?;

    emit_progress(&app, "qa", "🤖 AI 思考中", 30.0, "running", Some(&question));

    let answer = ai::qa_note(&app, &content, &question).await?;

    emit_progress(&app, "qa", "💾 写入笔记", 90.0, "running", None);

    if write_back {
        // Append to ## 问答记录 section
        let now = crate::commands::notes::timestamp_now();
        let qa_entry = format!(
            "\n\n### {now} — {question}\n\n> **问：** {question}\n\n> **答：** {answer}\n\n📍 参考章节：基于全文笔记\n"
        );

        let updated = if let Some(pos) = content.rfind("## 大衍决心得") {
            // Insert before 大衍决心得, after content
            let mut s = content[..pos].to_string();
            if let Some(qa_pos) = s.rfind("## 问答记录") {
                // Append to existing QA section
                s.insert_str(qa_pos + "## 问答记录".len(), &qa_entry);
            } else {
                // Create new QA section before 大衍决心得
                s.push_str(&format!("\n\n---\n\n## 问答记录\n{qa_entry}"));
            }
            s + &content[pos..]
        } else {
            // No 大衍决心得 section yet, just append
            format!("{content}\n\n---\n\n## 问答记录\n{qa_entry}")
        };

        std::fs::write(&note_path, &updated)
            .map_err(|e| AppError::Config(format!("写入笔记失败: {e}")))?;
    }

    emit_progress(
        &app,
        "qa",
        "追问完成",
        100.0,
        "completed",
        if write_back {
            Some("已追加到笔记")
        } else {
            Some("仅回答，未写入")
        },
    );

    Ok(answer)
}

fn check_deps(python_path: &str) -> Result<(), AppError> {
    let (program, prefix_args) = python_command_parts(python_path);
    let output = std::process::Command::new(program)
        .args(prefix_args)
        .args(["--version"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let v = format!(
                "{} {}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            if python_version_at_least(&v, PYTHON_MIN_VERSION) {
                return Ok(());
            }
        }
        _ => {}
    }
    Err(AppError::MissingDependency(format!(
        "Python 3.9+ 不可用: {python_path}"
    )))
}

fn python_version_at_least(output: &str, min: (u32, u32)) -> bool {
    parse_python_version_output(output)
        .map(|(major, minor, _)| (major, minor) >= min)
        .unwrap_or(false)
}

fn parse_python_version_output(output: &str) -> Option<(u32, u32, u32)> {
    for token in output.split_whitespace() {
        if let Some(version) = parse_python_version_token(token) {
            return Some(version);
        }
    }
    None
}

fn parse_python_version_token(token: &str) -> Option<(u32, u32, u32)> {
    let token = token.trim_start_matches("Python").trim();
    let mut parts = token.split('.');
    let major = parse_numeric_part(parts.next()?)?;
    let minor = parse_numeric_part(parts.next()?)?;
    let patch = parts.next().and_then(parse_numeric_part).unwrap_or(0);
    Some((major, minor, patch))
}

fn parse_numeric_part(part: &str) -> Option<u32> {
    let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn validate_pipeline_deps(mode: &InputMode, python_path: &str) -> Result<(), AppError> {
    let mut missing: Vec<String> = Vec::new();

    let needs_python = matches!(
        mode,
        InputMode::Bilibili
            | InputMode::Youtube
            | InputMode::Douyin
            | InputMode::Xiaohongshu
            | InputMode::LocalVideo
            | InputMode::LocalAudio
    );
    let needs_ytdlp = matches!(
        mode,
        InputMode::Bilibili | InputMode::Youtube | InputMode::Douyin | InputMode::Xiaohongshu
    );
    let needs_ffmpeg = matches!(
        mode,
        InputMode::Bilibili
            | InputMode::Youtube
            | InputMode::Douyin
            | InputMode::Xiaohongshu
            | InputMode::LocalVideo
    );
    let needs_asr = matches!(
        mode,
        InputMode::Bilibili
            | InputMode::Youtube
            | InputMode::Douyin
            | InputMode::Xiaohongshu
            | InputMode::LocalVideo
            | InputMode::LocalAudio
    );

    if needs_python && check_deps(python_path).is_err() {
        missing.push(format!(
            "Python 3.9+ 不可用。当前解析到: {python_path}。请在设置中指定真实 Python，避免 Windows Store python3 stub。"
        ));
    }

    if needs_ytdlp && !ytdlp_available(python_path) {
        missing.push(format!(
            "yt-dlp 不可用。请执行 `{python_path} -m pip install -U yt-dlp`，或安装 yt-dlp.exe。"
        ));
    }

    if needs_ffmpeg && resolve_ffmpeg_binary("ffmpeg").is_none() {
        missing.push(
            "FFmpeg 不可用。视频下载后的音频提取和关键帧截图需要 FFmpeg，请安装 `winget install Gyan.FFmpeg` 或把 ffmpeg.exe 加入 PATH。"
                .into(),
        );
    }

    if needs_asr && !python_modules_available(python_path, &["faster_whisper", "ctranslate2"]) {
        missing.push(format!(
            "faster-whisper 不可用。ASR 需要在当前 Python 中安装：`{python_path} -m pip install -U faster-whisper`。"
        ));
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(AppError::MissingDependency(format!(
            "\n{}",
            missing
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n")
        )))
    }
}

fn python_modules_available(python_path: &str, modules: &[&str]) -> bool {
    let import_list = modules.join(", ");
    let code = format!("import {import_list}");
    let (program, mut args) = python_command_parts(python_path);
    args.extend(["-c".into(), code]);
    command_works_strings(&program, &args)
}

fn ytdlp_available(python_path: &str) -> bool {
    let (program, mut args) = ytdlp_command(python_path);
    args.push("--version".into());
    command_works_strings(&program, &args)
}

fn command_works(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn command_works_strings(program: &str, args: &[String]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn apply_windows_no_window(cmd: &mut std::process::Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

fn resolve_ffmpeg_binary(name: &str) -> Option<String> {
    let candidates = if cfg!(target_os = "windows") {
        vec![format!("{name}.exe"), name.to_string()]
    } else {
        vec![name.to_string()]
    };

    for candidate in candidates {
        if command_works(&candidate, &["-version"]) {
            return Some(candidate);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_base = std::path::PathBuf::from(local_app_data)
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

fn ytdlp_command(python_path: &str) -> (String, Vec<String>) {
    for candidate in if cfg!(target_os = "windows") {
        vec!["yt-dlp.exe", "yt-dlp"]
    } else {
        vec!["yt-dlp"]
    } {
        if command_works(candidate, &["--version"]) {
            return (candidate.to_string(), vec![]);
        }
    }

    let (program, mut args) = python_command_parts(python_path);
    args.extend(["-m".into(), "yt_dlp".into()]);
    (program, args)
}

fn download_youtube_subtitles_with_python(
    python_path: &str,
    url: &str,
    output_dir: &std::path::Path,
) -> Result<PathBuf, AppError> {
    use crate::commands::python::run_python_script;

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            run_python_script(
                python_path,
                "download_youtube_subtitles.py",
                &[
                    url.to_string(),
                    "--output-dir".into(),
                    output_dir.to_string_lossy().to_string(),
                ],
            )
            .await
        })
    })?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: "download_youtube_subtitles".into(),
            stderr: result.stderr,
        });
    }

    let text_path = output_dir.join("text.txt");
    if text_path.exists() {
        Ok(text_path)
    } else {
        Err(AppError::Other(
            "YouTube 字幕脚本完成但未找到 text.txt".into(),
        ))
    }
}

/// 预取视频元信息（标题/作者/时长），不下载视频
fn fetch_video_metadata(python_path: &str, url: &str) -> Option<(String, String, f64)> {
    log::debug!(target: "agent","[metadata] yt-dlp --dump-json: {url}");
    let (program, prefix_args) = ytdlp_command(python_path);
    let mut cmd = std::process::Command::new(program);
    apply_windows_no_window(&mut cmd);
    cmd.args(prefix_args)
        .args(["--dump-json", "--no-playlist", url])
        .env("PYTHONUTF8", "1");
    let output = cmd.output().ok()?;

    if !output.status.success() {
        log::warn!(
            "[metadata] yt-dlp --dump-json failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(json_str.trim()).ok()?;

    let title = json["title"].as_str().unwrap_or("未知标题").to_string();
    let author = json["uploader"]
        .as_str()
        .or_else(|| json["channel"].as_str())
        .unwrap_or("未知作者")
        .to_string();
    let duration = json["duration"].as_f64().unwrap_or(0.0);

    log::debug!(target: "agent","[metadata] title={title}, author={author}, duration={duration:.0}s");
    Some((title, author, duration))
}

/// 调用 AI Douyin API 解析视频 URL，把响应 JSON 写入临时文件并返回路径
/// B站/抖音/小红书 走此路径；YouTube 直接用 yt-dlp
async fn resolve_via_ai_douyin(video_url: &str, temp_dir: &std::path::Path) -> Result<PathBuf, AppError> {
    // 读取 AI Douyin API Key（优先级：配置文件 > OS 密钥链）
    let douyin_key = match crate::commands::config::read_config_value("ai_douyin_api_key") {
        Some(key) => {
            log::debug!(target: "agent","[douyin] found api key in config file (len={})", key.len());
            key
        }
        None => {
            log::warn!("[douyin] ai_douyin_api_key not found in config file");
            return Err(AppError::Ai {
                kind: "provider_not_configured".into(),
                message: "未配置 AI Douyin API Key。请在设置 → API 密钥 中配置 ai_douyin_api_key。ai-douyin.top9.cc 注册获取免费额度。".into(),
            });
        }
    };

    let client = reqwest::Client::new();
    let resp = client
        .post("https://ai-douyin.top9.cc/api/v1/video/download-url")
        .header("X-API-Key", &douyin_key)
        .json(&serde_json::json!({"url": video_url}))
        .send()
        .await
        .map_err(|e| AppError::Config(format!("AI Douyin API 请求失败: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Config(format!(
            "AI Douyin API 返回 {status}: {body}"
        )));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| {
        AppError::Config(format!("AI Douyin API 响应解析失败: {e}"))
    })?;

    // 只记结构摘要（顶层 keys + 字符数），不记响应原文 ——
    // 响应 JSON 可能含下载直链、签名 URL、用户/会话标识等可重放凭证，
    // 全量 serde_json::to_string 入日志会被 Webview target 转发到前端 F12 console，
    // 违反设计文档 §五「日志绝不记录 API Key / 完整 prompt / 私密内容」红线。
    // 响应原文已落盘到 download_url.json（下方 1793 行），排查时直接看文件即可。
    let response_keys: Vec<&String> = json
        .as_object()
        .map(|o| o.keys().collect())
        .unwrap_or_default();
    let response_chars = serde_json::to_string(&json).map(|s| s.len()).unwrap_or(0);
    log::debug!(
        target: "agent",
        "[douyin] phase=response keys={response_keys:?} body_chars={response_chars}"
    );

    let json_path = temp_dir.join("download_url.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&json).unwrap_or_default())
        .map_err(AppError::Io)?;

    log::debug!(
        target: "agent",
        "[douyin] phase=resolved json_path={}",
        json_path.display()
    );
    Ok(json_path)
}

/// 通过 AI Douyin API 解析 → download_video_candidates.py 下载视频
/// B站/抖音/小红书 使用
async fn download_douyin_video(
    python_path: &str,
    video_url: &str,
    output: &std::path::Path,
    temp_dir: &std::path::Path,
) -> Result<String, AppError> {
    // 读取 Key（resolve_via_ai_douyin 内部也会读，这里需要再读一次传给脚本）
    let douyin_key = crate::commands::config::read_config_value("ai_douyin_api_key").unwrap_or_default();

    let json_path = resolve_via_ai_douyin(video_url, temp_dir).await?;

    let output_str = output.to_string_lossy().to_string();
    let json_str = json_path.to_string_lossy().to_string();
    log::debug!(target: "agent","[douyin] download_video_candidates.py: {json_str} → {output_str}");

    let mut args: Vec<String> = vec![
        "--response-json".into(), json_str,
        "--output".into(), output_str,
    ];
    if !douyin_key.is_empty() {
        args.push("--api-key".into());
        args.push(douyin_key);
    }

    let result = crate::commands::python::run_python_script(
        python_path,
        "download_video_candidates.py",
        &args,
    )
    .await?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: "download_video_candidates.py".into(),
            stderr: result.stderr,
        });
    }

    // Try to extract title from the API response (re-read from file)
    let title = if let Ok(json_str) = std::fs::read_to_string(&json_path) {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_str) {
            data.get("title")
                .or_else(|| data.get("desc"))
                .and_then(|v| v.as_str())
                .unwrap_or("抖音视频")
                .to_string()
        } else {
            "抖音视频".to_string()
        }
    } else {
        "抖音视频".to_string()
    };

    log::debug!(target: "agent","[douyin] download complete: {title}");
    Ok(title)
}

/// Download video using yt-dlp
fn download_video_ytdlp(
    python_path: &str,
    url: &str,
    _mode: &InputMode,
    output: &std::path::Path,
) -> Result<String, AppError> {
    let output_str = output.to_string_lossy();
    log::debug!(target: "agent","[download] yt-dlp: {url}");
    let is_bilibili = matches!(_mode, InputMode::Bilibili);
    let (program, prefix_args) = ytdlp_command(python_path);
    let mut cmd = std::process::Command::new(&program);
    apply_windows_no_window(&mut cmd);
    cmd.args(&prefix_args);
    cmd.args([
        "-o",
        &output_str,
        "--print",
        "%(title)s",
        "--no-playlist",
    ]);
    //  B站 412 风控：缺 Referer 头会被拒。加上的话大部分视频不需要 Cookie 就能下。
    if is_bilibili {
        cmd.arg("--add-header").arg("Referer:https://www.bilibili.com");
    }
    cmd.arg(url)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");

    let mut r = cmd
        .output()
        .map_err(|e| AppError::Config(format!("yt-dlp 未安装: {e}")))?;

    //  裸跑被 412 挡了？试 cookies-from-browser（B站受限内容需要登录态）。
    if !r.status.success() && is_bilibili {
        let stderr_first = String::from_utf8_lossy(&r.stderr);
        if stderr_first.contains("412") {
            log::warn!("[download] yt-dlp 裸跑 412，降级 --cookies-from-browser edge: {url}");
            let mut cmd2 = std::process::Command::new(&program);
            apply_windows_no_window(&mut cmd2);
            cmd2.args(&prefix_args);
            cmd2.args([
                "-o", &output_str,
                "--print", "%(title)s",
                "--no-playlist",
                "--add-header", "Referer:https://www.bilibili.com",
                "--cookies-from-browser", "edge",
                url,
            ])
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8");
            r = cmd2.output().map_err(|e| AppError::Config(format!("yt-dlp 未安装: {e}")))?;
        }
    }

    if !r.status.success() {
        let stderr = String::from_utf8_lossy(&r.stderr);
        log::error!("[download] yt-dlp failed: {stderr}");
        return Err(AppError::Config(format!("yt-dlp 下载失败: {stderr}")));
    }
    let stdout = String::from_utf8_lossy(&r.stdout);
    let title = stdout
        .lines()
        .last()
        .unwrap_or("未知标题")
        .trim()
        .to_string();
    log::debug!(target: "agent","[download] title: {title}");

    // 校验文件是否真的存在（yt-dlp 可能合并失败但仍返回 0）
    if !media_file_ready(output) {
        log::error!(
            "[download] yt-dlp 返回成功但文件不存在: {} ({} bytes?)",
            output.display(),
            output.metadata().map(|m| m.len()).unwrap_or(0)
        );
        return Err(AppError::Config(
            "yt-dlp 返回成功但视频文件未生成，可能是音视频合并失败。请确认 FFmpeg 已安装并加入 PATH。".into(),
        ));
    }
    Ok(title)
}

fn download_audio_ytdlp(
    python_path: &str,
    url: &str,
    output: &std::path::Path,
) -> Result<(), AppError> {
    let output_template = output.to_string_lossy();
    log::debug!(target: "agent","[download_audio] yt-dlp: {url}");
    let (program, prefix_args) = ytdlp_command(python_path);
    let mut cmd = std::process::Command::new(program);
    apply_windows_no_window(&mut cmd);
    cmd.args(prefix_args);
    cmd.args([
        "-f",
        "bestaudio/best",
        "-x",
        "--audio-format",
        "mp3",
        "--audio-quality",
        "0",
        "-o",
        &output_template,
        "--no-playlist",
        url,
    ])
    .env("PYTHONUTF8", "1")
    .env("PYTHONIOENCODING", "utf-8");

    let result = cmd
        .output()
        .map_err(|e| AppError::Config(format!("yt-dlp 音频下载无法启动: {e}")))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        log::error!("[download_audio] yt-dlp failed: {stderr}");
        return Err(AppError::Config(format!("yt-dlp 音频下载失败: {stderr}")));
    }

    if media_file_ready(output) {
        return Ok(());
    }

    // yt-dlp can append an extension when post-processing. Pick the first matching artifact.
    if let Some(parent) = output.parent() {
        let expected_stem = output.file_stem().and_then(|s| s.to_str()).unwrap_or("audio");
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let path = entry.path();
                let stem_matches = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|stem| stem == expected_stem)
                    .unwrap_or(false);
                if stem_matches && media_file_ready(&path) {
                    if path != output {
                        std::fs::copy(&path, output).map_err(AppError::Io)?;
                    }
                    return Ok(());
                }
            }
        }
    }

    Err(AppError::Other(format!(
        "yt-dlp 音频下载完成但未找到输出文件: {}",
        output.display()
    )))
}

fn media_file_ready(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn extract_audio_ffmpeg(video: &PathBuf, audio: &PathBuf) -> Result<(), AppError> {
    let ffmpeg = resolve_ffmpeg_binary("ffmpeg")
        .ok_or_else(|| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;
    let status = std::process::Command::new(ffmpeg)
        .args([
            "-i",
            &video.to_string_lossy(),
            "-q:a",
            "0",
            "-map",
            "a",
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
            )
            .await
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

/// 调用 extract_keyframes.py（支持可选的引导时间点），返回截图输出目录
/// 只使用 smart 模式：字幕引导时间点 + 场景变化检测，不使用固定间隔
fn extract_keyframes_guided(
    python_path: &str,
    video: &PathBuf,
    output_dir: &PathBuf,
    guided_timestamps: Option<&std::path::Path>,
) -> Result<PathBuf, AppError> {
    use crate::commands::python::run_python_script;

    if !video.exists() {
        return Err(AppError::Other("视频文件不存在".into()));
    }

    let mut args: Vec<String> = vec![
        "--video".into(),
        video.to_string_lossy().to_string(),
        "--output-dir".into(),
        output_dir.to_string_lossy().to_string(),
        // 强制 scene 模式（不使用固定间隔）
        "--mode".into(),
        "scene".into(),
        "--max-frames".into(),
        "40".into(),
        "--scene-threshold".into(),
        "0.25".into(),
        "--max-gap".into(),
        "120".into(),
        "--min-gap".into(),
        "3".into(),
    ];

    if let Some(ts_path) = guided_timestamps {
        if ts_path.exists() {
            args.push("--timestamps".into());
            args.push(ts_path.to_string_lossy().to_string());
            log::debug!(target: "agent",
                "[pipeline] keyframes extraction with guided timestamps: {}",
                ts_path.display()
            );
        }
    }

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(async { run_python_script(python_path, "extract_keyframes.py", &args).await })
    })?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: "extract_keyframes".into(),
            stderr: result.stderr,
        });
    }

    Ok(output_dir.clone())
}

/// 调用 extract_keyframes.py，返回截图输出目录
#[allow(dead_code)]
fn extract_keyframes_with_python(
    python_path: &str,
    video: &PathBuf,
    output_dir: &PathBuf,
) -> Result<PathBuf, AppError> {
    extract_keyframes_guided(python_path, video, output_dir, None)
}

/// 获取视频时长（秒），失败返回 0
fn get_video_duration(video_path: &std::path::Path) -> f64 {
    if !video_path.exists() {
        return 0.0;
    }

    let Some(ffmpeg_bin) = resolve_ffmpeg_binary("ffprobe") else {
        return 0.0;
    };

    let output = std::process::Command::new(ffmpeg_bin)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            &video_path.to_string_lossy(),
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.trim().parse::<f64>().unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

/// 判断是否为在线视频模式（需要时间戳链接）
fn is_online_video(mode: &InputMode) -> bool {
    matches!(
        mode,
        InputMode::Bilibili | InputMode::Youtube | InputMode::Douyin | InputMode::Xiaohongshu
    )
}

fn generate_temp_id(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.hash(&mut h);
    format!("{:x}", h.finish())
}
