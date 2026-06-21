// ============================================================
// MindEngine — 统一 AI 调用入口
// 职责: 读密钥链 → 调 DeepSeekClient → 返回响应
// 所有提示词外置到 prompts/ 目录，由 PromptManager 运行时渲染（见 prompt_manager.rs）
// ============================================================

use super::deepseek::stream_deepseek;
use super::prompt_manager::PromptManager;
use super::types::{MindRequest, MindResponse};
use crate::error::AppError;
use tauri::AppHandle;

/// 从多处读取 DeepSeek API Key（优先级：环境变量 > 配置文件）
///
/// 红线（设计文档 §五）：日志只记「找到/未找到」+ 来源，**绝不**记 key 值本身。
pub fn read_deepseek_key() -> Result<String, AppError> {
    // 1. 环境变量（最高优先级，CI/容器场景）
    for env_name in &["MYRIAD_DEEPSEEK_API_KEY", "DEEPSEEK_API_KEY"] {
        if let Ok(key) = std::env::var(env_name) {
            if !key.is_empty() {
                // 只记来源 + 长度，不记 key 值
                log::debug!(
                    target: "agent",
                    "[engine] key=found source=env name={env_name} key_len={}",
                    key.len()
                );
                return Ok(key);
            }
        }
    }

    // 2. 配置文件 ~/.myriad-mind-app/config.json
    if let Some(key) = crate::commands::config::read_config_value("deepseek_api_key") {
        log::debug!(
            target: "agent",
            "[engine] key=found source=config_file key_len={}",
            key.len()
        );
        return Ok(key);
    }

    log::warn!(target: "agent", "[engine] key=not_found source=none reason=no_env_and_no_config");
    Err(AppError::Ai {
        kind: "provider_not_configured".into(),
        message: "未找到 DeepSeek API Key。请在设置中配置，或设置环境变量 DEEPSEEK_API_KEY。"
            .into(),
    })
}

/// 运行 AI 任务 — Tauri command
#[tauri::command]
pub async fn run_mind_task(
    app_handle: AppHandle,
    request: MindRequest,
) -> Result<MindResponse, AppError> {
    // ---- 模型路由埋点（设计文档 §五）：入口侧记录 task + 是否显式 override
    // 最终选模型（Pro/Flash + reason）的日志在 deepseek.rs::pick_model / stream_deepseek 中。
    log::debug!(
        target: "agent",
        "[engine] phase=enter fn=run_mind_task task={} model_override={} stream={}",
        request.task,
        request.model_override.as_deref().unwrap_or("none"),
        request.stream
    );
    let api_key = read_deepseek_key()?;
    stream_deepseek(&app_handle, &request, &api_key).await
}

/// 便捷函数：用 DeepSeek V4 Pro 生成学习笔记
/// 供 pipeline.rs 等内部模块调用
pub async fn generate_note(
    app_handle: &AppHandle,
    content: &str,
    content_type: &str,
    note_dir: Option<&str>,    // 输出目录，用于读取 memory.md
    task_prompt: Option<&str>, // 用户本次要求
) -> Result<String, AppError> {
    let api_key = read_deepseek_key()?;

    // 读取知识库记忆
    let memory_context = if let Some(dir) = note_dir {
        let memory_path = std::path::PathBuf::from(dir)
            .join(".myriad-mind")
            .join("memory.md");
        if memory_path.exists() {
            std::fs::read_to_string(&memory_path).unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // 内容模式标志（对应 prompts/note/system.md 模板里的 {% if is_video %} 等分支）
    let is_video = content_type.contains("视频");
    let is_code = content_type.contains("代码") || content_type.contains("code");
    let is_article = content_type.contains("网页") || content_type.contains("文章");

    // 用 PromptManager 渲染 system prompt（模板文件在 prompts/note/）
    let pm = PromptManager::new()?;
    let mode_key = content_mode_key(content_type);
    // 模板路由决策埋点：content_type → 哪个 mode_*.md 模板
    log::debug!(
        target: "agent",
        "[engine] phase=route fn=generate_note content_type={content_type:?} mode_key={mode_key} \
         is_video={is_video} is_code={is_code} is_article={is_article} has_task_prompt={} has_memory={}",
        task_prompt.map(str::trim).filter(|p| !p.is_empty()).is_some(),
        !memory_context.is_empty()
    );
    let mode_contract = pm.render(&format!("note/mode_{mode_key}"), ())?;
    let task_prompt_value = task_prompt.map(str::trim).filter(|p| !p.is_empty());
    let system_prompt = pm.render(
        "note/system",
        minijinja::context! {
            is_video => is_video,
            is_code => is_code,
            is_article => is_article,
            mode_contract => &mode_contract,
            memory_context => &memory_context,
            task_prompt => task_prompt_value,
            content_type => content_type,
        },
    )?;

    let req = MindRequest {
        task: super::types::AiTask::NoteGeneration,
        messages: vec![super::types::AiMessage {
            role: "user".into(),
            content: content.to_string(),
        }],
        system_prompt,
        model_override: None,
        stream: true,
        max_tokens: Some(65536),
        thinking: Some(super::types::ThinkingConfig {
            enabled: true,
            effort: Some(super::types::ReasoningEffort::High),
        }),
    };

    let resp = stream_deepseek(app_handle, &req, &api_key).await?;
    log::debug!(
        target: "agent",
        "[engine] phase=done fn=generate_note output_chars={} model={} finish_reason={:?}",
        resp.text.len(),
        resp.model,
        resp.finish_reason
    );
    Ok(resp.text)
}

/// content_type → 模板 mode_key 映射
///
/// 显式化原本隐式散落在代码里的字符串 contains 契约：去掉"更新·"前缀后，
/// 按内容类型映射到 prompts/note/mode_{key}.md 模板。
fn content_mode_key(content_type: &str) -> &'static str {
    let ct = content_type.trim_start_matches("更新·");
    if ct.contains("代码") || ct.contains("code") {
        "code"
    } else if ct.contains("网页") || ct.contains("文章") {
        "article"
    } else if ct.contains("文档") || ct.contains("文本") {
        "document"
    } else if ct.contains("短文") {
        "short"
    } else {
        "video" // 默认（"视频" 及未匹配的情况）
    }
}

#[allow(dead_code)]
pub async fn qa_note(
    app_handle: &AppHandle,
    note_content: &str,
    question: &str,
) -> Result<String, AppError> {
    let api_key = read_deepseek_key()?;
    let pm = PromptManager::new()?;
    let system_prompt = pm.render("qa", minijinja::context! { note_content => note_content })?;

    let req = MindRequest {
        task: super::types::AiTask::NoteGeneration,
        messages: vec![super::types::AiMessage {
            role: "user".into(),
            content: question.to_string(),
        }],
        system_prompt,
        model_override: Some("deepseek-v4-flash".into()),
        stream: true,
        max_tokens: Some(4096),
        thinking: None,
    };

    let resp = stream_deepseek(app_handle, &req, &api_key).await?;
    Ok(resp.text)
}

/// 测试 DeepSeek 连接
#[tauri::command]
pub async fn test_deepseek_connection(app_handle: AppHandle) -> Result<String, AppError> {
    let api_key = read_deepseek_key()?;
    let pm = PromptManager::new()?;
    let system_prompt = pm.render("ping", ())?;
    let req = MindRequest {
        task: super::types::AiTask::Summary,
        messages: vec![super::types::AiMessage {
            role: "user".into(),
            content: "回复\"pong\"".into(),
        }],
        system_prompt,
        model_override: Some("deepseek-v4-flash".into()),
        stream: false,
        max_tokens: Some(10),
        thinking: None,
    };

    let response = stream_deepseek(&app_handle, &req, &api_key).await?;
    Ok(format!("pong — {}", response.model))
}
