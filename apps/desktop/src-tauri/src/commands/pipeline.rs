// ============================================================
// 管线编排 — 多步骤管线执行器
// 串联 Python 脚本 + MindEngine AI，每步推送进度事件
// ============================================================

use crate::commands::ai;
use crate::commands::python::{python_command_parts, resolve_python_path};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
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

    // 步骤 1-8: 调度 agent（目标驱动，AI 在六阶段骨架内自主选工具，取代原 4 分流管线）
    let _ = debug_metadata; // 保留 IPC 参数（前端传入）；agent 自带 generation trace
    let agent_req = crate::agent::AgentRequest {
        input: input.clone(),
        note_dir: note_dir.clone(),
        python_path: py.clone(),
        task_prompt: task_prompt.clone(),
        allow_paid: true, // v1 默认放开；后续接 config features 花费开关
    };
    let agent_result = match crate::agent::run(&app, agent_req).await {
        Ok(r) => r,
        Err(AppError::Cancelled) => {
            emit_progress(&app, "cancelled", "已取消", 0.0, "failed", None);
            return Err(AppError::Cancelled);
        }
        Err(e) => {
            emit_progress(&app, "error", &format!("agent 失败: {e}"), 0.0, "failed", None);
            return Err(e);
        }
    };
    log::info!(
        target: "agent",
        "[pipeline] agent done steps={} tools={:?} tokens={}",
        agent_result.steps,
        agent_result.tools_used,
        agent_result.total_tokens
    );

    // 步骤 9: 清理临时文件——先于持久化执行，确保 persist_note 失败也不会泄漏 GB 级 temp。
    // （persist_note 只读内存里的 note_content，不依赖 temp_dir，故清理顺序无副作用）
    let should_cleanup = cleanup_temp.unwrap_or(true);
    let video_id = generate_temp_id(&input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&video_id);
    if should_cleanup && temp_dir.exists() {
        emit_progress(&app, "cleanup", "清理临时文件", 92.0, "running", None);
        let _ = std::fs::remove_dir_all(&temp_dir);
        emit_progress(&app, "cleanup", "清理完成", 94.0, "completed", None);
    } else if !should_cleanup && temp_dir.exists() {
        emit_progress(
            &app,
            "cleanup",
            "保留临时文件（按设置跳过清理）",
            94.0,
            "completed",
            Some(&format!("临时目录: {}", temp_dir.display())),
        );
    }

    // 持久化笔记（解析 `> ai_category:` 决定子目录；标题取首行 #）
    emit_progress(&app, "save", "💾 保存笔记", 95.0, "running", None);
    let note_path = persist_note(&note_dir, note_category.as_deref(), &agent_result.note_content)?;
    emit_progress(&app, "save", "笔记已保存", 98.0, "completed", Some(&note_path));

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
        note_path: Some(note_path),
        error: None,
        duration_seconds: duration,
    })
}

/// 把 agent 产出的笔记写入 note_dir。
/// 子目录：用户显式 note_category > 笔记末尾 `> ai_category:` 行 > "未分类"。
/// 文件名：首行 `# ` 标题 > "笔记"。同名追加 -N 去重。
///
/// 注：v1 不更新 .myriad-mind/ 知识库索引与指纹（原管线有），作为已知回归，后续补 library 接入。
fn persist_note(
    note_dir: &str,
    override_category: Option<&str>,
    content: &str,
) -> Result<String, AppError> {
    let category = override_category
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            // 兼容半角 `:` 与全角 `：`（中文 LLM 易产出全角冒号，否则分类静默回落"未分类"）
            content.lines().find_map(|l| {
                let l = l.trim();
                let rest = l.strip_prefix("> ai_category")?;
                let rest = rest.trim_start_matches(&[':', '：'][..]).trim().to_string();
                if rest.is_empty() {
                    None
                } else {
                    Some(rest)
                }
            })
        })
        .unwrap_or_else(|| "未分类".to_string());

    let title = content
        .lines()
        .find_map(|l| l.trim().strip_prefix("# ").map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "笔记".to_string());
    let slug = sanitize_filename(&title);

    let dir = PathBuf::from(note_dir).join(&category);
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;

    let mut path = dir.join(format!("{slug}.md"));
    let mut i = 1;
    while path.exists() {
        path = dir.join(format!("{slug}-{i}.md"));
        i += 1;
    }
    std::fs::write(&path, content).map_err(AppError::Io)?;
    Ok(path.to_string_lossy().to_string())
}

/// 文件名净化：剔除路径非法字符，限长 60。
fn sanitize_filename(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .take(60)
        .collect();
    cleaned.trim().to_string()
}

// ============================================================
// 工具函数
// ============================================================

pub(crate) fn emit_progress(
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

// ============================================================
// 依赖校验
// ============================================================

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

// ============================================================
// 媒体下载 / 处理（agent tools 复用）
// ============================================================

/// 调用 AI Douyin API 解析视频 URL，把响应 JSON 写入临时文件并返回路径
/// B站/抖音/小红书 走此路径；YouTube 直接用 yt-dlp
pub(crate) async fn resolve_via_ai_douyin(video_url: &str, temp_dir: &std::path::Path) -> Result<PathBuf, AppError> {
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
pub(crate) async fn download_douyin_video(
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
pub(crate) fn download_video_ytdlp(
    python_path: &str,
    url: &str,
    _mode: &InputMode,
    output: &std::path::Path,
) -> Result<String, AppError> {
    let output_str = output.to_string_lossy();
    log::debug!(target: "agent","[download] yt-dlp: {url}");
    let is_bilibili = matches!(_mode, InputMode::Bilibili);
    let (program, prefix_args) = ytdlp_command(python_path);

    // 可选：用户配置的 cookies.txt 路径（B站/YouTube 登录态）
    let cookies_file: Option<std::path::PathBuf> =
        crate::commands::config::read_config_value("ytdlp_cookies_file")
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from);

    let mut cmd = std::process::Command::new(&program);
    apply_windows_no_window(&mut cmd);
    cmd.args(&prefix_args);
    cmd.args([
        "--ignore-config", // 忽略用户全局配置，防止全局配置里写了 --cookies-from-browser 导致锁库报错
        "-o",
        &output_str,
        "--print",
        "%(title)s",
        "--no-playlist",
        "--remote-components",
        "ejs:github",
        "--extractor-args",
        "youtube:player_client=android,web",
        "--sleep-requests",
        "3",
        "--sleep-interval",
        "5",
        "-f",
        "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best",
    ]);
    //  B站 412 风控：缺 Referer 头会被拒。加上的话大部分视频不需要 Cookie 就能下。
    if is_bilibili {
        cmd.arg("--add-header").arg("Referer:https://www.bilibili.com");
    }
    if let Some(ref cf) = cookies_file {
        cmd.arg("--cookies").arg(cf);
    }
    cmd.arg(url)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8");

    let mut r = cmd
        .output()
        .map_err(|e| AppError::Config(format!("yt-dlp 未安装: {e}")))?;

    //  裸跑被 412 挡了？尝试带登录态重跑。
    if !r.status.success() && is_bilibili {
        let stderr_first = String::from_utf8_lossy(&r.stderr);
        if stderr_first.contains("412") {
            let has_cookies_file = cookies_file.is_some();
            if has_cookies_file {
                log::warn!("[download] yt-dlp 裸跑 412，使用配置 cookies 文件重试: {url}");
            } else {
                log::warn!("[download] yt-dlp 裸跑 412，降级 --cookies-from-browser edge: {url}");
            }
            let mut cmd2 = std::process::Command::new(&program);
            apply_windows_no_window(&mut cmd2);
            cmd2.args(&prefix_args);
            cmd2.args([
                "--ignore-config",
                "-o",
                &output_str,
                "--print",
                "%(title)s",
                "--no-playlist",
                "--remote-components",
                "ejs:github",
                "-f",
                "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best",
                "--add-header",
                "Referer:https://www.bilibili.com",
            ]);
            if let Some(ref cf) = cookies_file {
                cmd2.arg("--cookies").arg(cf);
            } else {
                cmd2.arg("--cookies-from-browser").arg("edge");
            }
            cmd2.arg(url)
                .env("PYTHONUTF8", "1")
                .env("PYTHONIOENCODING", "utf-8");
            r = cmd2
                .output()
                .map_err(|e| AppError::Config(format!("yt-dlp 未安装: {e}")))?;
        }
    }

    if !r.status.success() {
        let stderr = String::from_utf8_lossy(&r.stderr);
        log::error!("[download] yt-dlp failed: {stderr}");
        let user_msg = if stderr.contains("Could not copy") {
            "yt-dlp 无法复制浏览器 Cookie 数据库（浏览器可能正在运行）。\n\
             解决方案（任选其一）：\n\
             1. 关闭 Chrome/Edge 后重试；\n\
             2. 在设置 → 配置文件中添加 ytdlp_cookies_file 指向导出的 cookies.txt；\n\
             3. 配置 AI Douyin API Key，B站优先走 AI Douyin 解析。"
                .into()
        } else if stderr.contains("412") {
            "B站返回 412，需要登录态。\n\
             解决方案（任选其一）：\n\
             1. 配置 AI Douyin API Key（推荐）；\n\
             2. 在设置 → 配置文件中添加 ytdlp_cookies_file 指向 B站登录后的 cookies.txt；\n\
             3. 关闭 Chrome/Edge 后重试，让 yt-dlp 读取浏览器 Cookie。"
                .into()
        } else {
            format!("yt-dlp 下载失败: {stderr}")
        };
        return Err(AppError::Config(user_msg));
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

pub(crate) fn media_file_ready(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

pub(crate) fn extract_audio_ffmpeg(video: &PathBuf, audio: &PathBuf) -> Result<(), AppError> {
    if !video.exists() {
        return Err(AppError::Other(format!(
            "视频文件不存在: {}",
            video.display()
        )));
    }

    let ffmpeg = resolve_ffmpeg_binary("ffmpeg")
        .ok_or_else(|| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;

    // 1. 先用 ffprobe 检查是否存在音频流，避免 ffmpeg -map a 对无音频视频报错
    if let Some(ffprobe) = resolve_ffmpeg_binary("ffprobe") {
        let probe = std::process::Command::new(ffprobe)
            .args([
                "-v", "error",
                "-select_streams", "a",
                "-show_entries", "stream=codec_type",
                "-of", "csv=p=0",
                &video.to_string_lossy(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        match probe {
            Ok(output) if output.status.success() => {
                let has_audio = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line.trim().eq_ignore_ascii_case("audio"));
                if !has_audio {
                    return Err(AppError::Other(
                        "该视频没有音频流，无法提取音频进行 ASR 转写。".into(),
                    ));
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::warn!(
                    target: "agent",
                    "[ffmpeg] ffprobe 检查音频流失败: {stderr}"
                );
                // 不阻断，继续尝试 ffmpeg
            }
            Err(e) => {
                log::warn!(
                    target: "agent",
                    "[ffmpeg] 无法启动 ffprobe: {e}"
                );
                // 不阻断，继续尝试 ffmpeg
            }
        }
    }

    // 2. 提取音频（捕获 stderr 以便诊断）
    let output = std::process::Command::new(ffmpeg)
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
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| AppError::MissingDependency(format!("FFmpeg 无法启动: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_summary: String = stderr.chars().take(500).collect();
        log::error!(target: "agent", "[ffmpeg] 音频提取失败: {stderr_summary}");

        let user_msg = if stderr.contains("does not contain any stream")
            || stderr.contains("Stream map 'a'")
        {
            "该视频没有音频流，无法提取音频进行 ASR 转写。".into()
        } else {
            format!("FFmpeg 音频提取失败: {stderr_summary}")
        };
        return Err(AppError::Other(user_msg));
    }
    Ok(())
}

pub(crate) fn generate_temp_id(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.hash(&mut h);
    format!("{:x}", h.finish())
}
