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
    let mut cmd = std::process::Command::new(&program);
    apply_windows_no_window(&mut cmd);
    cmd.args(&prefix_args);
    cmd.args([
        "-o",
        &output_str,
        "--print",
        "%(title)s",
        "--no-playlist",
        "--remote-components", "ejs:github",
        "--extractor-args", "youtube:player_client=android,web",
        "--sleep-requests", "3", "--sleep-interval", "5",
        "-f", "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best",
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
                "--remote-components", "ejs:github",
                "-f", "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/best",
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

pub(crate) fn media_file_ready(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

pub(crate) fn extract_audio_ffmpeg(video: &PathBuf, audio: &PathBuf) -> Result<(), AppError> {
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

// ============================================================
// 关键帧提取 — Rust 原生 FFmpeg 调用（替代 extract_keyframes.py）
// 复刻 Python 脚本三模式：guided / interval / scene，参数与输出格式完全一致
// ============================================================

/// 关键帧提取结果（Tauri 命令返回，从 python.rs 迁移）
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeResult {
    pub result: KeyframeData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeData {
    pub video_path: String,
    pub output_dir: String,
    pub mode: String,
    pub interval: u32,
    pub max_frames: u32,
    pub keyframes: Vec<KeyframeInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeInfo {
    pub file: String,
    pub timestamp_seconds: f64,
    pub timestamp_label: String,
}

/// 内部帧条目（含 trigger 字段，写入 keyframes.json 供 Vision 审查读取）
struct KeyframeEntry {
    file: String,
    timestamp_seconds: f64,
    timestamp_label: String,
    trigger: String,
}

/// 时间戳标签 — 复刻 Python `timestamp_label`：`03m03s` / `01h03m03s`
fn kf_timestamp_label(seconds: f64) -> String {
    let s = std::cmp::max(0, seconds.round() as i64);
    let (minutes, sec) = (s / 60, s % 60);
    let (hours, min) = (minutes / 60, minutes % 60);
    if hours > 0 {
        format!("{hours:02}h{min:02}m{sec:02}s")
    } else {
        format!("{min:02}m{sec:02}s")
    }
}

/// reason 转文件名安全 slug — 复刻 Python `slug_reason`：
/// 空白转 `_`，保留 \w / CJK / `_` / `-`，截断 24 字符，空则 `guided`
fn kf_slug_reason(reason: &str) -> String {
    let cleaned: String = reason
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    let filtered: String = cleaned
        .chars()
        .filter(|c| {
            c.is_alphanumeric() || matches!(*c, '_' | '-') || ('\u{4e00}'..='\u{9fff}').contains(c)
        })
        .collect();
    let truncated: String = filtered.chars().take(24).collect();
    if truncated.is_empty() {
        "guided".to_string()
    } else {
        truncated
    }
}

/// 加载引导时间戳 JSON — 复刻 Python `load_guided_timestamps`：
/// 接受 `{timestamps: [...]}` 或裸数组，元素为数字或 `{ts, reason}`，
/// 按 ts 排序去重（2 秒间隔），返回 `(ts, reason)` 列表
fn load_guided_timestamps(path: Option<&std::path::Path>) -> Vec<(f64, String)> {
    let path = match path {
        Some(p) if p.exists() => p,
        _ => return Vec::new(),
    };

    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let raw: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let items = raw.get("timestamps").unwrap_or(&raw);
    let arr = match items.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut result: Vec<(f64, String)> = Vec::new();
    for item in arr {
        if let Some(num) = item.as_f64() {
            if num >= 0.0 {
                result.push((num, "AI推荐".to_string()));
            }
        } else if let Some(obj) = item.as_object() {
            let ts = obj
                .get("ts")
                .or_else(|| obj.get("timestamp"))
                .or_else(|| obj.get("timestamp_seconds"))
                .and_then(|v| v.as_f64());
            if let Some(ts) = ts {
                if ts >= 0.0 {
                    let reason = obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("AI推荐")
                        .to_string();
                    result.push((ts, reason));
                }
            }
        }
    }

    result.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut deduped: Vec<(f64, String)> = Vec::new();
    for (ts, reason) in result {
        if deduped
            .last()
            .map_or(false, |(last_ts, _)| (ts - last_ts).abs() < 2.0)
        {
            continue;
        }
        deduped.push((ts, reason));
    }
    deduped
}

/// 引导截图 — 按指定时间点逐帧提取，复刻 Python `extract_guided_frames`
fn extract_guided_frames(
    video: &std::path::Path,
    frames_dir: &std::path::Path,
    timestamps: &[(f64, String)],
    max_frames: usize,
    ffmpeg: &str,
) -> Result<Vec<KeyframeEntry>, AppError> {
    for (index, (ts, reason)) in timestamps.iter().take(max_frames).enumerate() {
        let idx = index + 1;
        let slug = kf_slug_reason(reason);
        let output = frames_dir.join(format!("guided_{idx:04}_{slug}.png"));
        let mut cmd = std::process::Command::new(ffmpeg);
        apply_windows_no_window(&mut cmd);
        let status = cmd
            .args([
                "-ss",
                &format!("{ts:.3}"),
                "-i",
                &video.to_string_lossy(),
                "-frames:v",
                "1",
                "-q:v",
                "2",
                "-y",
                &output.to_string_lossy(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| AppError::Other(format!("FFmpeg 引导截图执行失败: {e}")))?;

        if !status.success() {
            log::warn!(
                "[keyframes] guided frame {idx} at {ts:.3}s failed (exit {:?})",
                status.code()
            );
        }
    }

    // 收集生成的 guided 帧
    let mut keyframes = Vec::new();
    for (index, (ts, reason)) in timestamps.iter().take(max_frames).enumerate() {
        let idx = index + 1;
        let slug = kf_slug_reason(reason);
        let output = frames_dir.join(format!("guided_{idx:04}_{slug}.png"));
        if output.exists() {
            keyframes.push(KeyframeEntry {
                file: output
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                timestamp_seconds: *ts,
                timestamp_label: kf_timestamp_label(*ts),
                trigger: format!("guided:{reason}"),
            });
        }
    }
    Ok(keyframes)
}

/// 间隔截图 — 固定间隔提取，复刻 Python `extract_interval_frames`
fn extract_interval_frames(
    video: &std::path::Path,
    frames_dir: &std::path::Path,
    interval: u32,
    max_frames: u32,
    ffmpeg: &str,
) -> Result<Vec<KeyframeEntry>, AppError> {
    let fps = 1.0 / interval as f64;
    let filter_v = format!("fps={fps:.6}");
    let pattern = frames_dir.join("frame_%04d.png");

    let mut cmd = std::process::Command::new(ffmpeg);
    apply_windows_no_window(&mut cmd);
    let status = cmd
        .args([
            "-i",
            &video.to_string_lossy(),
            "-vf",
            &filter_v,
            "-frames:v",
            &max_frames.to_string(),
            "-q:v",
            "2",
            "-y",
            &pattern.to_string_lossy(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| AppError::Other(format!("FFmpeg 间隔截图执行失败: {e}")))?;

    if !status.success() {
        return Err(AppError::Other("FFmpeg 间隔截图失败".into()));
    }

    // 收集生成的 frame_*.png，按编号排序
    let mut keyframes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(frames_dir) {
        let mut pngs: Vec<_> = entries
            .flatten()
            .filter(|e| {
                e.path().extension().and_then(|x| x.to_str()) == Some("png")
                    && e.file_name().to_string_lossy().starts_with("frame_")
            })
            .collect();
        pngs.sort_by_key(|e| e.file_name().to_os_string());
        for png in pngs {
            let stem = png
                .path()
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let num: u32 = stem
                .split('_')
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let ts = num.saturating_sub(1) as f64 * interval as f64;
            keyframes.push(KeyframeEntry {
                file: png.file_name().to_string_lossy().to_string(),
                timestamp_seconds: ts,
                timestamp_label: kf_timestamp_label(ts),
                trigger: "interval".into(),
            });
        }
    }
    Ok(keyframes)
}

/// 场景检测截图 — 两遍法，复刻 Python `extract_scene_frames`：
/// Pass1 `showinfo` 检测场景变化 pts_time；Pass2 逐帧精确截图
fn extract_scene_frames(
    video: &std::path::Path,
    frames_dir: &std::path::Path,
    max_frames: usize,
    ffmpeg: &str,
    threshold: f64,
    min_gap: f64,
    _max_gap: f64,
) -> Result<Vec<KeyframeEntry>, AppError> {
    let null_dev = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };
    let filter_v = format!("select=gt(scene\\,{threshold}),showinfo");

    // Pass 1: 检测场景变化 pts_time
    let mut cmd = std::process::Command::new(ffmpeg);
    apply_windows_no_window(&mut cmd);
    let output = cmd
        .args([
            "-i",
            &video.to_string_lossy(),
            "-vf",
            &filter_v,
            "-vsync",
            "vfr",
            "-f",
            "null",
            "-y",
            null_dev,
        ])
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .output()
        .map_err(|e| AppError::Other(format!("FFmpeg 场景检测执行失败: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut pts_times: Vec<f64> = Vec::new();
    for line in stderr.lines() {
        if let Some(pos) = line.find("pts_time:") {
            let rest = &line[pos + "pts_time:".len()..];
            let num_str: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(pts) = num_str.parse::<f64>() {
                pts_times.push(pts);
            }
        }
    }

    if pts_times.is_empty() {
        return Ok(Vec::new());
    }

    // 去重：enforce min_gap
    let mut deduped: Vec<f64> = Vec::new();
    let mut last_ts = -min_gap - 1.0;
    for pts in pts_times {
        if pts - last_ts < min_gap {
            continue;
        }
        deduped.push(pts);
        last_ts = pts;
    }

    let timestamps: Vec<f64> = deduped.into_iter().take(max_frames).collect();

    // Pass 2: 逐帧截图
    let mut keyframes = Vec::new();
    for (index, ts) in timestamps.iter().enumerate() {
        let idx = index + 1;
        let safe_tag = format!("{ts:.1}s").replace('.', "_");
        let output_path = frames_dir.join(format!("scene_{idx:04}_{safe_tag}.png"));
        let mut cmd = std::process::Command::new(ffmpeg);
        apply_windows_no_window(&mut cmd);
        let status = cmd
            .args([
                "-ss",
                &format!("{ts:.3}"),
                "-i",
                &video.to_string_lossy(),
                "-frames:v",
                "1",
                "-q:v",
                "2",
                "-y",
                &output_path.to_string_lossy(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| AppError::Other(format!("FFmpeg 场景截图执行失败: {e}")))?;

        if !status.success() {
            log::warn!(
                "[keyframes] scene frame {idx} at {ts:.3}s failed (exit {:?})",
                status.code()
            );
        }
        if output_path.exists() {
            keyframes.push(KeyframeEntry {
                file: output_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                timestamp_seconds: *ts,
                timestamp_label: kf_timestamp_label(*ts),
                trigger: "scene".into(),
            });
        }
    }
    Ok(keyframes)
}

/// 原生关键帧提取（替代 extract_keyframes.py）
///
/// 参数与 Python 脚本 CLI 完全一致，输出 `output_dir/frames/keyframes.json` + PNG。
/// keyframes.json 包含 `trigger` 字段（供 Vision 审查读取），与 Python 脚本输出格式一致。
fn extract_keyframes_native(
    video: &std::path::Path,
    output_dir: &std::path::Path,
    mode: &str,
    interval: u32,
    max_frames: u32,
    timestamps_path: Option<&std::path::Path>,
    scene_threshold: f64,
    min_gap: f64,
    max_gap: f64,
) -> Result<KeyframeResult, AppError> {
    let ffmpeg = resolve_ffmpeg_binary("ffmpeg")
        .ok_or_else(|| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;

    if !video.exists() {
        return Err(AppError::Other(format!(
            "视频文件不存在: {}",
            video.display()
        )));
    }

    let frames_dir = output_dir.join("frames");
    std::fs::create_dir_all(&frames_dir).map_err(AppError::Io)?;

    let mut all_keyframes: Vec<KeyframeEntry> = Vec::new();

    // 引导时间点
    let guided = load_guided_timestamps(timestamps_path);
    if !guided.is_empty() {
        log::debug!(target: "agent", "[keyframes] phase=guided count={}", guided.len());
        let guided_frames = extract_guided_frames(
            video,
            &frames_dir,
            &guided,
            max_frames as usize,
            &ffmpeg,
        )?;
        all_keyframes.extend(guided_frames);
    }

    // 间隔模式
    if mode == "interval" || mode == "both" {
        log::debug!(target: "agent", "[keyframes] phase=interval interval={interval}");
        let interval_frames =
            extract_interval_frames(video, &frames_dir, interval, max_frames, &ffmpeg)?;
        all_keyframes.extend(interval_frames);
    }

    // 场景模式
    if mode == "scene" || mode == "both" {
        let scene_max = if mode == "scene" {
            max_frames as usize
        } else {
            std::cmp::max(max_frames as usize / 3, 5)
        };
        log::debug!(target: "agent", "[keyframes] phase=scene threshold={scene_threshold} max={scene_max}");
        let scene_frames = extract_scene_frames(
            video,
            &frames_dir,
            scene_max,
            &ffmpeg,
            scene_threshold,
            min_gap,
            max_gap,
        )?;
        all_keyframes.extend(scene_frames);
    }

    // 去重（按文件名）并按时间戳排序
    all_keyframes.sort_by(|a, b| {
        a.timestamp_seconds
            .partial_cmp(&b.timestamp_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen_files = std::collections::HashSet::new();
    let mut unique: Vec<KeyframeEntry> = Vec::new();
    for kf in all_keyframes {
        if seen_files.insert(kf.file.clone()) {
            unique.push(kf);
        }
    }

    // 限制最大帧数
    unique.truncate(max_frames as usize);

    // 写 keyframes.json（含 trigger 字段，与 Python 脚本输出一致）
    let index_data: Vec<serde_json::Value> = unique
        .iter()
        .map(|kf| {
            serde_json::json!({
                "file": kf.file,
                "timestamp_seconds": kf.timestamp_seconds,
                "timestamp_label": kf.timestamp_label,
                "trigger": kf.trigger,
            })
        })
        .collect();
    let index_path = frames_dir.join("keyframes.json");
    std::fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_data).unwrap_or_else(|_| "[]".into()),
    )
    .map_err(AppError::Io)?;

    log::debug!(
        target: "agent",
        "[keyframes] phase=done frames={} index={}",
        unique.len(),
        index_path.display()
    );

    let keyframes: Vec<KeyframeInfo> = unique
        .into_iter()
        .map(|kf| KeyframeInfo {
            file: kf.file,
            timestamp_seconds: kf.timestamp_seconds,
            timestamp_label: kf.timestamp_label,
        })
        .collect();

    Ok(KeyframeResult {
        result: KeyframeData {
            video_path: video.to_string_lossy().to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            mode: mode.to_string(),
            interval,
            max_frames,
            keyframes,
        },
    })
}

/// 关键帧提取（Tauri 命令，从 python.rs 迁移）
///
/// 替代原 `python.rs::extract_keyframes`（Python 子进程中转）。
/// `python_path` 参数保留以维持前端 IPC 契约，但不再使用。
#[tauri::command]
pub async fn extract_keyframes(
    video_path: String,
    output_dir: String,
    #[allow(unused_variables)] python_path: String,
    interval: u32,
    max_frames: u32,
    mode: String,
) -> Result<KeyframeResult, AppError> {
    let video = PathBuf::from(&video_path);
    let output = PathBuf::from(&output_dir);
    extract_keyframes_native(
        &video,
        &output,
        &mode,
        interval,
        max_frames,
        None,
        0.25,
        3.0,
        120.0,
    )
}

/// 调用原生关键帧提取（支持可选的引导时间点），返回截图输出目录
/// 只使用 smart 模式：字幕引导时间点 + 场景变化检测，不使用固定间隔
pub(crate) fn extract_keyframes_guided(
    #[allow(unused_variables)] python_path: &str,
    video: &PathBuf,
    output_dir: &PathBuf,
    guided_timestamps: Option<&std::path::Path>,
) -> Result<PathBuf, AppError> {
    if !video.exists() {
        return Err(AppError::Other("视频文件不存在".into()));
    }

    if let Some(ts_path) = guided_timestamps {
        if ts_path.exists() {
            log::debug!(
                target: "agent",
                "[pipeline] keyframes extraction with guided timestamps: {}",
                ts_path.display()
            );
        }
    }

    log::debug!(target: "agent", "[pipeline] keyframes extraction (native FFmpeg)");

    extract_keyframes_native(
        video,
        output_dir,
        "scene",
        30,
        40,
        guided_timestamps,
        0.25,
        3.0,
        120.0,
    )?;

    Ok(output_dir.clone())
}

pub(crate) fn generate_temp_id(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    input.hash(&mut h);
    format!("{:x}", h.finish())
}

#[cfg(test)]
mod keyframe_tests {
    use super::*;

    #[test]
    fn timestamp_label_minutes() {
        assert_eq!(kf_timestamp_label(0.0), "00m00s");
        assert_eq!(kf_timestamp_label(63.0), "01m03s");
        assert_eq!(kf_timestamp_label(183.4), "03m03s");
    }

    #[test]
    fn timestamp_label_hours() {
        assert_eq!(kf_timestamp_label(3661.0), "01h01m01s");
        assert_eq!(kf_timestamp_label(7385.0), "02h03m05s");
    }

    #[test]
    fn timestamp_label_negative_clamps_to_zero() {
        assert_eq!(kf_timestamp_label(-5.0), "00m00s");
    }

    #[test]
    fn slug_reason_chinese() {
        assert_eq!(kf_slug_reason("AI推荐"), "AI推荐");
        assert_eq!(kf_slug_reason("开场 白"), "开场_白");
    }

    #[test]
    fn slug_reason_empty_falls_back() {
        assert_eq!(kf_slug_reason(""), "guided");
        assert_eq!(kf_slug_reason("!!!"), "guided");
    }

    #[test]
    fn slug_reason_truncates_to_24() {
        let long = "a".repeat(30);
        let result = kf_slug_reason(&long);
        assert_eq!(result.len(), 24);
    }

    #[test]
    fn load_guided_timestamps_none_path() {
        let result = load_guided_timestamps(None);
        assert!(result.is_empty());
    }

    #[test]
    fn load_guided_timestamps_nonexistent_path() {
        let result = load_guided_timestamps(Some(std::path::Path::new("/nonexistent/file.json")));
        assert!(result.is_empty());
    }

    #[test]
    fn load_guided_timestamps_dedup_2sec() {
        let dir = std::env::temp_dir().join("kf_test_dedup.json");
        let json = r#"[{"ts": 10.0, "reason": "A"}, {"ts": 11.5, "reason": "B"}, {"ts": 20.0, "reason": "C"}]"#;
        std::fs::write(&dir, json).unwrap();
        let result = load_guided_timestamps(Some(&dir));
        std::fs::remove_file(&dir).ok();
        // 10.0 and 11.5 are within 2 sec → deduped; 20.0 kept
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, 10.0);
        assert_eq!(result[1].0, 20.0);
    }

    #[test]
    fn load_guided_timestamps_array_of_numbers() {
        let dir = std::env::temp_dir().join("kf_test_nums.json");
        let json = r#"[5.0, 15.0, 30.0]"#;
        std::fs::write(&dir, json).unwrap();
        let result = load_guided_timestamps(Some(&dir));
        std::fs::remove_file(&dir).ok();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].1, "AI推荐");
    }
}
