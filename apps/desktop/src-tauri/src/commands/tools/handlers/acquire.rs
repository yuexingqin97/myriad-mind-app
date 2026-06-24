// ============================================================
// Acquire 阶段工具 — 原始材料采集（URL 抓取 / 视频下载 / 音频提取 / ASR /
// 字幕下载 / 代码扫描 / 文件读取 / 目录扫描 / AI Douyin 任务查询）
//
// 所有 handler 复用现有 commands 底层逻辑，phase = Phase::Acquire。
// 大文本结果（文章正文 / 转写文本 / 字幕 / 代码扫描）落盘到 artifacts_dir，
// 只回 ArtifactRef；小结果（小文件 / 目录树 / 任务列表）直接 ToolOutput::text。
// ============================================================

use crate::commands::code_project::{format_code_project_for_ai, scan_code_project};
use crate::commands::config::read_config_value;
use crate::commands::fetch::{fetch_article, format_article_for_ai};
use crate::commands::fs::{read_text_file, scan_directory};
use crate::commands::pipeline::{
    download_douyin_video, download_video_ytdlp, extract_audio_ffmpeg, InputMode,
};
use crate::commands::ai_douyin::list_ai_douyin_tasks;
use crate::commands::python::{download_youtube_subtitles, transcribe_audio};
use crate::commands::tools::{
    ArtifactKind, ArtifactRef, Cost, Phase, ToolContext, ToolFuture, ToolHandler, ToolOutput,
    ToolSpec, opt_str, require_str,
};
use crate::error::AppError;
use std::path::PathBuf;

// ------------------------------------------------------------
// fetch_url — 抓取网页 → Markdown 正文 artifact
// ------------------------------------------------------------

/// 抓取在线文章 URL，提取结构化正文，落盘 article.md 返回 ArticleText artifact。
pub struct FetchUrlHandler;

impl ToolHandler for FetchUrlHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "fetch_url".into(),
            description: "抓取网页文章 URL（知乎/CSDN/掘金/简书/Wiki/通用），提取标题/作者/正文为 Markdown，落盘为 article.md artifact。反爬平台会返回友好错误。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "文章完整 URL（http(s):// 开头）"
                    }
                },
                "required": ["url"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let url = require_str(&params, "url")?;

            // 2. 抓取并格式化为 AI Markdown
            let article = fetch_article(&url).await?;
            let title = article.title.clone();
            let author = article.author.clone();
            let platform = article.platform.clone();
            let markdown = format_article_for_ai(&article);
            let char_count = markdown.chars().count();

            // 3. 落盘到 artifacts_dir/article.md
            ctx.ensure_artifacts_dir()?;
            let path = ctx.artifacts_dir.join("article.md");
            std::fs::write(&path, &markdown).map_err(AppError::Io)?;

            // 4. 构造 artifact 引用 + 摘要（含标题/作者/平台/字数）
            let summary = format!(
                "《{title}》 · 作者 {} · 平台 {platform} · {char_count} 字",
                author.as_deref().unwrap_or("未知"),
            );
            let art = ArtifactRef {
                id: "article.md".into(),
                path,
                kind: ArtifactKind::ArticleText,
                tokens_estimate: ArtifactRef::estimate_tokens(&markdown),
                summary,
            };

            Ok(ToolOutput::artifact(
                format!("已抓取文章：{title}（{char_count} 字）"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// download_video — 在线视频下载（路由 youtube → ytdlp / 其余 → AI Douyin）
// ------------------------------------------------------------

/// 下载在线视频。YouTube 走 yt-dlp；B站/抖音/小红书等走 AI Douyin API。
/// 输出到 temp_dir/video.mp4，返回 VideoFile artifact。
pub struct DownloadVideoHandler;

impl ToolHandler for DownloadVideoHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "download_video".into(),
            description: "下载在线视频（YouTube 用 yt-dlp，B站/抖音/小红书用 AI Douyin API）。输出 temp_dir/video.mp4，返回视频标题。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "视频 URL（youtube/bilibili/douyin/xiaohongshu）"
                    }
                },
                "required": ["url"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let url = require_str(&params, "url")?;

            // 2. 输出路径
            let output = ctx.temp_dir.join("video.mp4");
            std::fs::create_dir_all(&ctx.temp_dir).map_err(AppError::Io)?;

            // 3. 路由：url 含 youtube → yt-dlp；否则（bilibili/douyin/xhs）→ AI Douyin
            let lower = url.to_lowercase();
            let title = if lower.contains("youtube") || lower.contains("youtu.be") {
                // YouTube → yt-dlp（同步函数）
                download_video_ytdlp(
                    &ctx.python_path,
                    &url,
                    &InputMode::Youtube,
                    &output,
                )?
            } else {
                // bilibili/douyin/xhs → AI Douyin API（异步）
                let mode = if lower.contains("bilibili") || lower.contains("b23.tv") {
                    InputMode::Bilibili
                } else {
                    InputMode::Douyin
                };
                // 先尝试 AI Douyin；B站失败回退 yt-dlp（与 pipeline.rs 路由一致）
                match download_douyin_video(
                    &ctx.python_path,
                    &url,
                    &output,
                    &ctx.temp_dir,
                )
                .await
                {
                    Ok(t) => t,
                    Err(e) if matches!(mode, InputMode::Bilibili) => {
                        log::warn!(
                            target: "agent",
                            "[tool:download_video] ai_douyin failed for bilibili, fallback ytdlp: {e}"
                        );
                        download_video_ytdlp(&ctx.python_path, &url, &InputMode::Bilibili, &output)?
                    }
                    Err(e) => return Err(e),
                }
            };

            // 4. VideoFile artifact（视频文件本身不计 token）
            let art = ArtifactRef {
                id: "video.mp4".into(),
                path: output,
                kind: ArtifactKind::VideoFile,
                tokens_estimate: 0,
                summary: title.clone(),
            };

            Ok(ToolOutput::artifact(
                format!("已下载视频：{title}"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// extract_audio — 从视频提取音频 (FFmpeg)
// ------------------------------------------------------------

/// 用 FFmpeg 从视频文件提取 mp3 音频，输出 temp_dir/audio.mp3，返回 AudioFile artifact。
pub struct ExtractAudioHandler;

impl ToolHandler for ExtractAudioHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "extract_audio".into(),
            description: "用 FFmpeg 从本地视频文件提取音频（mp3），输出 temp_dir/audio.mp3。用于后续 ASR 转写。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "video_path": {
                        "type": "string",
                        "description": "视频文件绝对路径"
                    }
                },
                "required": ["video_path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let video_path = require_str(&params, "video_path")?;
            let video = PathBuf::from(&video_path);

            // 2. 提取音频（FFmpeg 同步）
            std::fs::create_dir_all(&ctx.temp_dir).map_err(AppError::Io)?;
            let audio = ctx.temp_dir.join("audio.mp3");
            extract_audio_ffmpeg(&video, &audio)?;

            // 3. AudioFile artifact
            let art = ArtifactRef {
                id: "audio.mp3".into(),
                path: audio,
                kind: ArtifactKind::AudioFile,
                tokens_estimate: 0,
                summary: format!("音频（来自 {}）", video.display()),
            };

            Ok(ToolOutput::artifact("音频提取完成", art))
        })
    }
}

// ------------------------------------------------------------
// transcribe_asr — 音频 ASR 转写 (faster-whisper)
// ------------------------------------------------------------

/// 调用 faster-whisper 转写音频，文本落盘 transcript.txt，返回 Transcript artifact。
pub struct TranscribeAsrHandler;

impl ToolHandler for TranscribeAsrHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "transcribe_asr".into(),
            description: "用 faster-whisper 把音频转写为文本（支持多语言自动检测）。落盘 transcript.txt，摘要含语言/段数。model_size 越大越准越慢（tiny/base/small/medium/large）。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "audio_path": {
                        "type": "string",
                        "description": "音频文件绝对路径"
                    },
                    "model_size": {
                        "type": "string",
                        "description": "whisper 模型大小，默认 small",
                        "default": "small"
                    },
                    "device": {
                        "type": "string",
                        "description": "推理设备 cpu/cuda，默认 cpu",
                        "default": "cpu"
                    }
                },
                "required": ["audio_path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数（model_size / device 可选，默认 small / cpu）
            let audio_path = require_str(&params, "audio_path")?;
            let model_size = opt_str(&params, "model_size").unwrap_or_else(|| "small".into());
            let device = opt_str(&params, "device").unwrap_or_else(|| "cpu".into());

            // 2. 转写（异步；output_dir 用 temp_dir，python 脚本写 text.txt/srt）
            let output_dir = ctx.temp_dir.to_string_lossy().to_string();
            let result = transcribe_audio(
                audio_path,
                output_dir.clone(),
                ctx.python_path.clone(),
                model_size,
                device,
            )
            .await?;

            // 3. 读取 text_path 全文，落盘到 artifacts_dir/transcript.txt
            let text_path = &result.result.text_path;
            let text = std::fs::read_to_string(text_path)
                .map_err(|e| AppError::Other(format!("读取转写文本失败 {}: {e}", text_path)))?;

            ctx.ensure_artifacts_dir()?;
            let art_path = ctx.artifacts_dir.join("transcript.txt");
            std::fs::write(&art_path, &text).map_err(AppError::Io)?;

            // 4. Transcript artifact + 摘要（语言/段数）
            let language = result.result.language.clone();
            let segment_count = result.result.segment_count;
            let art = ArtifactRef {
                id: "transcript.txt".into(),
                path: art_path,
                kind: ArtifactKind::Transcript,
                tokens_estimate: ArtifactRef::estimate_tokens(&text),
                summary: format!("{language} · {segment_count} 段"),
            };

            Ok(ToolOutput::artifact(
                format!("转写完成：{language}，{segment_count} 段"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// download_subtitles — YouTube 字幕下载
// ------------------------------------------------------------

/// 下载 YouTube 字幕。有文本则落盘 subtitle.txt 返回 Subtitle artifact；全无则文本提示。
pub struct DownloadSubtitlesHandler;

impl ToolHandler for DownloadSubtitlesHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "download_subtitles".into(),
            description: "下载 YouTube 视频字幕（自动字幕 + 人工字幕）。有可用字幕则落盘 subtitle.txt 返回 Subtitle artifact；无则返回提示文本。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "YouTube 视频 URL"
                    },
                    "languages": {
                        "type": "string",
                        "description": "可选，优先语言代码逗号分隔，如 \"zh,en\""
                    }
                },
                "required": ["url"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let url = require_str(&params, "url")?;
            let languages = opt_str(&params, "languages");

            // 2. 下载字幕（输出目录用 temp_dir）
            std::fs::create_dir_all(&ctx.temp_dir).map_err(AppError::Io)?;
            let output_dir = ctx.temp_dir.to_string_lossy().to_string();
            let subtitle = download_youtube_subtitles(
                url,
                output_dir,
                ctx.python_path.clone(),
                languages,
            )
            .await?;

            // 3. text_path 可能 None（视频无字幕）→ 返回纯文本提示
            let text_path = match &subtitle.result.text_path {
                Some(p) => p,
                None => return Ok(ToolOutput::text("该视频无可用字幕")),
            };

            // 4. 读全文落盘 artifacts_dir/subtitle.txt
            let text = std::fs::read_to_string(text_path).map_err(|e| {
                AppError::Other(format!("读取字幕文件失败 {}: {e}", text_path))
            })?;
            ctx.ensure_artifacts_dir()?;
            let art_path = ctx.artifacts_dir.join("subtitle.txt");
            std::fs::write(&art_path, &text).map_err(AppError::Io)?;

            let langs = subtitle.result.languages.join(",");
            let art = ArtifactRef {
                id: "subtitle.txt".into(),
                path: art_path,
                kind: ArtifactKind::Subtitle,
                tokens_estimate: ArtifactRef::estimate_tokens(&text),
                summary: format!("字幕语言: {langs}"),
            };

            Ok(ToolOutput::artifact(
                format!("字幕下载完成（语言 {langs}）"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// scan_code_project — 代码项目扫描 → Markdown artifact
// ------------------------------------------------------------

/// 扫描代码项目目录，按优先级读取关键文件并格式化为 Markdown，落盘 code_scan.md。
pub struct ScanCodeProjectHandler;

impl ToolHandler for ScanCodeProjectHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "scan_code_project".into(),
            description: "扫描本地代码项目目录：递归建目录树、按优先级（README/构建配置/入口/核心源码）读取关键文件、推断技术栈，格式化为 Markdown 落盘 code_scan.md。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "项目根目录绝对路径"
                    }
                },
                "required": ["path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let path = require_str(&params, "path")?;
            // 沙箱：限制在可信读取根（input_root/temp/artifacts/note_dir）内
            let root = ctx.resolve_readable(&path)?;

            // 2. 扫描（max_depth=5）+ 格式化
            let scan = scan_code_project(&root, 5)?;
            let tech_stack = scan.tech_stack.join(", ");
            let total_files = scan.total_files;
            let markdown = format_code_project_for_ai(&scan);

            // 3. 落盘 artifacts_dir/code_scan.md
            ctx.ensure_artifacts_dir()?;
            let art_path = ctx.artifacts_dir.join("code_scan.md");
            std::fs::write(&art_path, &markdown).map_err(AppError::Io)?;

            // 4. CodeScan artifact + 摘要（技术栈/文件数）
            let art = ArtifactRef {
                id: "code_scan.md".into(),
                path: art_path,
                kind: ArtifactKind::CodeScan,
                tokens_estimate: ArtifactRef::estimate_tokens(&markdown),
                summary: format!("{tech_stack} · {total_files} 文件"),
            };

            Ok(ToolOutput::artifact(
                format!("代码扫描完成：{tech_stack}，{total_files} 文件"),
                art,
            ))
        })
    }
}

// ------------------------------------------------------------
// read_file — 读小文本文件（< 8000 字符直接回，否则截断预览）
// ------------------------------------------------------------

/// 读取文本文件。小文件（< 8000 字符）直接返回全文；大文件返回前 2000 字符预览 + 提示。
pub struct ReadFileHandler;

impl ToolHandler for ReadFileHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".into(),
            description: "读取本地文本文件（.md/.txt/.json/源码等）。小文件直接返回全文；大文件返回前 2000 字符预览并提示用 read_artifact 读取完整内容。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "文件绝对路径"
                    }
                },
                "required": ["path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let path = require_str(&params, "path")?;
            // 沙箱：限制在可信读取根内（防 prompt injection 读敏感文件）
            let resolved = ctx.resolve_readable(&path)?;

            // 2. 读全文（复用 fs::read_text_file 命令）
            let content = read_text_file(resolved.to_string_lossy().to_string()).await?;
            let char_count = content.chars().count();

            // 3. 小文件直接回；大文件截断前 2000 字符预览
            const SMALL_LIMIT: usize = 8_000;
            const PREVIEW: usize = 2_000;
            let output = if char_count <= SMALL_LIMIT {
                content
            } else {
                let head: String = content.chars().take(PREVIEW).collect();
                format!(
                    "{head}\n\n(共 {char_count} 字符，已截断预览。完整内容用 read_artifact 读取。)"
                )
            };

            Ok(ToolOutput::text(output))
        })
    }
}

// ------------------------------------------------------------
// scan_directory — 目录扫描 → 文本摘要（不落盘）
// ------------------------------------------------------------

/// 扫描目录下可处理文件（视频/音频/文本），格式化目录树为文本摘要返回。
pub struct ScanDirectoryHandler;

impl ToolHandler for ScanDirectoryHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "scan_directory".into(),
            description: "扫描目录下可处理文件（视频/音频/文本，递归最多 2 层），按类型分组返回文本摘要。不落盘 artifact。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "目录绝对路径"
                    }
                },
                "required": ["path"]
            }),
            phase: Phase::Acquire,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let path = require_str(&params, "path")?;
            // 沙箱：限制在可信读取根内
            let resolved = ctx.resolve_readable(&path)?;

            // 2. 扫描（复用 fs::scan_directory，内部递归 2 层 + 文件类型分类）
            let result = scan_directory(resolved.to_string_lossy().to_string()).await?;

            // 3. 格式化目录树为文本摘要（目录树通常不大，不落盘 artifact）
            let mut lines = Vec::with_capacity(result.files.len() + 4);
            lines.push(format!("目录: {}", result.path));
            lines.push(format!("共 {} 个可处理文件", result.total_count));
            // 按类型分组统计
            let mut groups: std::collections::BTreeMap<&str, usize> =
                std::collections::BTreeMap::new();
            for f in &result.files {
                *groups.entry(f.file_type.as_str()).or_default() += 1;
            }
            for (kind, n) in &groups {
                lines.push(format!("  - {kind}: {n}"));
            }
            lines.push(String::new());
            for f in &result.files {
                lines.push(format!("[{}] {} ({} B)", f.file_type, f.path, f.size_bytes));
            }
            let summary = lines.join("\n");

            Ok(ToolOutput::text(summary))
        })
    }
}

// ------------------------------------------------------------
// query_ai_douyin — 查询 AI Douyin 任务列表（付费）
// ------------------------------------------------------------

/// 查询 AI Douyin 任务列表。api_key 缺失返回配置错误；
/// 调用失败脱敏（stderr 可能含明文 key）。
pub struct QueryAiDouyinHandler;

impl ToolHandler for QueryAiDouyinHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "query_ai_douyin".into(),
            description: "查询 AI Douyin 平台已提交的视频解析任务列表。支持按 search/status 过滤。需要配置 ai_douyin_api_key（设置 → API 密钥）。调用失败时不回显 stderr（避免泄露密钥）。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "search": {
                        "type": "string",
                        "description": "可选，按关键词过滤任务标题/描述"
                    },
                    "status": {
                        "type": "string",
                        "description": "可选，按状态过滤（如 success/processing/failed）"
                    }
                },
                "required": []
            }),
            phase: Phase::Acquire,
            cost: Cost::Paid,
        }
    }

    fn handle<'a>(&'a self, _ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 取 api_key（缺失即配置错误，不脱敏，因为不涉及上游响应）
            let api_key = read_config_value("ai_douyin_api_key").ok_or_else(|| {
                AppError::Other("未配置 ai_douyin_api_key（请在设置 → API 密钥中配置）".into())
            })?;

            // 2. 解析可选过滤参数
            let search = opt_str(&params, "search");
            let status = opt_str(&params, "status");

            // 3. 查询（Rust 直连 reqwest，无 Python 中转；失败统一脱敏不回喂）
            let list = list_ai_douyin_tasks(
                api_key,
                None, // api_base 用默认 https://ai-douyin.top9.cc
                None, // page
                None, // page_size
                status,
                search,
            )
            .await
            .map_err(|e| {
                // reqwest 直连不再经 argv 回显 --api-key；但保守起见仍不回原始错误原文，
                // 只给脱敏提示；详情走开发者日志。
                log::warn!(
                    target: "agent",
                    "[tool:query_ai_douyin] failed (redacted): {e}"
                );
                AppError::Other("AI Douyin 查询失败（已脱敏，详情见日志文件）".into())
            })?;

            // 4. 成功：data 是上游 API 返回的 JSON，格式化为文本摘要
            let data = &list.data;
            let summary = match data {
                serde_json::Value::Array(items) => {
                    let n = items.len();
                    let mut lines = Vec::with_capacity(n + 1);
                    lines.push(format!("AI Douyin 任务列表：共 {n} 条"));
                    for (i, item) in items.iter().take(50).enumerate() {
                        let title = item
                            .get("title")
                            .or_else(|| item.get("desc"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("(无标题)");
                        let st = item
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        lines.push(format!("{}. [{st}] {title}", i + 1));
                    }
                    if n > 50 {
                        lines.push(format!("…（其余 {} 条已省略）", n - 50));
                    }
                    lines.join("\n")
                }
                other => {
                    // 非 array（对象 / 单值）兜底：序列化为紧凑 JSON
                    format!("AI Douyin 响应：{}", other)
                }
            };

            Ok(ToolOutput::text(summary))
        })
    }
}
