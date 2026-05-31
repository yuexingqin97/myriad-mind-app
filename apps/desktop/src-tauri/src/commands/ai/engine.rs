// ============================================================
// MindEngine — 统一 AI 调用入口
// 职责: 读密钥链 → 调 DeepSeekClient → 返回响应
// ============================================================

use super::deepseek::stream_deepseek;
use super::types::{MindRequest, MindResponse};
use crate::error::AppError;
use tauri::AppHandle;

/// 从 OS 密钥链读取 DeepSeek API Key
fn read_deepseek_key() -> Result<String, AppError> {
    // 1. 尝试 OS 密钥链 (Windows Credential Manager)
    #[cfg(target_os = "windows")]
    {
        use crate::commands::config::cred_read;
        if let Ok(Some(key)) = cred_read("deepseek-api-key") {
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }

    // 2. 环境变量兜底
    for env_name in &["MYRIAD_DEEPSEEK_API_KEY", "DEEPSEEK_API_KEY"] {
        if let Ok(key) = std::env::var(env_name) {
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }

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
    let api_key = read_deepseek_key()?;
    stream_deepseek(&app_handle, &request, &api_key).await
}

/// 便捷函数：用 DeepSeek V4 Pro 生成学习笔记
/// 供 pipeline.rs 等内部模块调用
pub async fn generate_note(
    app_handle: &AppHandle,
    content: &str,
    content_type: &str,
    note_dir: Option<&str>,       // 输出目录，用于读取 memory.md
    task_prompt: Option<&str>,    // 用户本次要求
) -> Result<String, AppError> {
    let api_key = read_deepseek_key()?;

    // 读取知识库记忆
    let memory_context = if let Some(dir) = note_dir {
        let memory_path = std::path::PathBuf::from(dir).join(".myriad-mind").join("memory.md");
        if memory_path.exists() {
            std::fs::read_to_string(&memory_path).unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // 构建 system prompt
    let mut system_prompt = String::from(
        "你是一个专业的学习笔记生成器。请将以下内容整理成结构化的学习笔记。\n\
        用 Markdown 格式输出，包含以下章节：\n\
        1. AI 摘要（2-3句话概括）\n\
        2. 核心概念（要点列表）\n\
        3. 详细笔记（按主题分段）\n\
        4. 关键术语表\n\
        5. 扩展学习建议\n\
        \n\
        输出语言：中文。专业术语保留英文原名。\n"
    );

    // 注入知识库记忆
    if !memory_context.is_empty() {
        system_prompt.push_str(&format!(
            "\n## 当前知识库已有内容（供参考，避免重复）\n\n{memory_context}\n\n\
            注意：如果本次内容与已有笔记相似或重复，优先更新已有笔记而非创建重复内容。\n"
        ));
    }

    // 注入本次要求
    if let Some(prompt) = task_prompt {
        if !prompt.trim().is_empty() {
            system_prompt.push_str(&format!(
                "\n## 用户本次特别要求\n\n{prompt}\n\n请严格遵守以上要求。\n"
            ));
        }
    }

    system_prompt.push_str(&format!("\n内容类型：{content_type}"));

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
    log::info!("[mind-engine] generated {} chars", resp.text.len());
    Ok(resp.text)
}

/// 测试 DeepSeek 连接
#[tauri::command]
pub async fn test_deepseek_connection(app_handle: AppHandle) -> Result<String, AppError> {
    let api_key = read_deepseek_key()?;
    let req = MindRequest {
        task: super::types::AiTask::Summary,
        messages: vec![super::types::AiMessage {
            role: "user".into(),
            content: "回复\"pong\"".into(),
        }],
        system_prompt: "回复 pong，不要回复其他内容。".into(),
        model_override: Some("deepseek-v4-flash".into()),
        stream: false,
        max_tokens: Some(10),
        thinking: None,
    };

    let response = stream_deepseek(&app_handle, &req, &api_key).await?;
    Ok(format!("pong — {}", response.model))
}
