// ============================================================
// DeepSeekClient — OpenAI-compatible HTTP + SSE 流式解析
// ============================================================

use super::types::{
    AiErrorKind, MindRequest, MindResponse, MindStreamEvent, ReasoningEffort, TokenUsage,
};
use crate::error::AppError;
use reqwest::Client;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio_stream::StreamExt;

const BASE_URL: &str = "https://api.deepseek.com";
const CHAT_ENDPOINT: &str = "/chat/completions";

/// 选择模型：Pro vs Flash
fn pick_model(request: &MindRequest) -> &str {
    if let Some(ref m) = request.model_override {
        return m;
    }
    match request.task {
        super::types::AiTask::Summary
        | super::types::AiTask::Translation
        | super::types::AiTask::NextStepSuggestion
        | super::types::AiTask::ResourceRecommend => "deepseek-v4-flash",
        _ => "deepseek-v4-pro",
    }
}

/// 构建 OpenAI Chat Completions 请求体
fn build_body(request: &MindRequest, model: &str) -> Value {
    let mut messages: Vec<Value> = vec![];

    // system prompt
    messages.push(serde_json::json!({
        "role": "system",
        "content": request.system_prompt,
    }));

    // user messages
    for msg in &request.messages {
        messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }

    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": request.stream,
        "max_tokens": request.max_tokens.unwrap_or(65536),
    });

    // thinking mode
    if let Some(ref thinking) = request.thinking {
        if thinking.enabled {
            body["thinking"] = serde_json::json!({ "type": "enabled" });
            if let Some(ref effort) = thinking.effort {
                body["reasoning_effort"] = match effort {
                    ReasoningEffort::High => serde_json::json!("high"),
                    ReasoningEffort::Max => serde_json::json!("max"),
                };
            }
        }
    }

    body
}

/// 流式调用 DeepSeek API
pub async fn stream_deepseek(
    app_handle: &AppHandle,
    request: &MindRequest,
    api_key: &str,
) -> Result<MindResponse, AppError> {
    let client = Client::new();
    let model = pick_model(request);
    let body = build_body(request, model);

    let url = format!("{BASE_URL}{CHAT_ENDPOINT}");

    log::info!(
        "[mind-engine] request: model={model}, task={}",
        request.task
    );

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let kind = if e.is_timeout() {
                "timeout".to_string()
            } else if e.is_connect() {
                "network".to_string()
            } else {
                "network".to_string()
            };
            AppError::Ai {
                kind: kind.to_string(),
                message: e.to_string(),
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let kind = AiErrorKind::classify(Some(status.as_u16()), &body_text);
        return Err(AppError::Ai {
            kind: kind.to_string(),
            message: body_text,
        });
    }

    // 推送 start 事件
    let _ = app_handle.emit(
        "mind-stream",
        MindStreamEvent::Start {
            task: request.task.to_string(),
            provider: "deepseek".into(),
            model: model.into(),
        },
    );

    let mut stream = response.bytes_stream();
    let mut full_text = String::new();
    let mut reasoning_text = String::new();
    let mut usage: Option<TokenUsage> = None;
    let mut finish_reason: Option<String> = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Ai {
            kind: "network".to_string(),
            message: format!("流读取失败: {e}"),
        })?;

        for line in chunk.split(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(line);
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if let Some(data) = trimmed.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }

                if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                    // 提取 choices[0].delta.content
                    if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                        full_text.push_str(content);
                        let _ = app_handle.emit(
                            "mind-stream",
                            MindStreamEvent::Delta {
                                delta: content.to_string(),
                            },
                        );
                    }

                    // 提取 reasoning_content（单独累计，不推送前端 UI）
                    if let Some(reasoning) =
                        parsed["choices"][0]["delta"]["reasoning_content"].as_str()
                    {
                        reasoning_text.push_str(reasoning);
                        let _ = app_handle.emit(
                            "mind-stream",
                            MindStreamEvent::ReasoningDelta {
                                delta: reasoning.to_string(),
                            },
                        );
                    }

                    // 提取 finish_reason
                    if let Some(fr) = parsed["choices"][0]["finish_reason"].as_str() {
                        finish_reason = Some(fr.to_string());
                    }

                    // 提取 usage（通常出现在最后一条消息中）
                    if let Some(u) = parsed.get("usage") {
                        usage = Some(TokenUsage {
                            input_tokens: u["input_tokens"].as_u64().unwrap_or(0) as u32,
                            output_tokens: u["output_tokens"].as_u64().unwrap_or(0) as u32,
                            reasoning_tokens: u["completion_tokens_details"]
                                .get("reasoning_tokens")
                                .and_then(|v| v.as_u64())
                                .map(|v| v as u32),
                            total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
                        });

                        let _ = app_handle.emit(
                            "mind-stream",
                            MindStreamEvent::Usage {
                                input_tokens: usage.as_ref().map(|u| u.input_tokens),
                                output_tokens: usage.as_ref().map(|u| u.output_tokens),
                                reasoning_tokens: usage.as_ref().and_then(|u| u.reasoning_tokens),
                                total_tokens: usage.as_ref().map(|u| u.total_tokens),
                            },
                        );
                    }
                }
            }
        }
    }

    // 推送 done 事件
    let _ = app_handle.emit(
        "mind-stream",
        MindStreamEvent::Done {
            text: full_text.clone(),
            finish_reason: finish_reason.clone(),
        },
    );

    log::info!(
        "[mind-engine] done: model={model}, text={}chars, reasoning={}chars",
        full_text.len(),
        reasoning_text.len(),
    );

    Ok(MindResponse {
        text: full_text,
        reasoning_text: if reasoning_text.is_empty() {
            None
        } else {
            Some(reasoning_text)
        },
        provider: "deepseek".into(),
        model: model.into(),
        usage,
        finish_reason,
    })
}

// ============================================================
// Vision API — 非流式图片理解调用
// ============================================================

use super::types::VisionRequest;

/// 调用 DeepSeek V4 Vision（非流式），返回文本响应
pub async fn vision_complete(request: &VisionRequest, api_key: &str) -> Result<String, AppError> {
    let client = Client::new();
    let model = request
        .model_override
        .as_deref()
        .unwrap_or("deepseek-v4-flash");

    // 构建 messages（OpenAI 兼容多模态格式）
    let mut messages: Vec<Value> = vec![serde_json::json!({
        "role": "system",
        "content": request.system_prompt,
    })];

    for msg in &request.messages {
        let content_blocks: Vec<Value> = msg
            .content
            .iter()
            .map(|block| match block {
                super::types::VisionContentBlock::Text { text } => {
                    serde_json::json!({"type": "text", "text": text})
                }
                super::types::VisionContentBlock::ImageUrl { image_url } => {
                    serde_json::json!({
                        "type": "image_url",
                        "image_url": {
                            "url": image_url.url,
                            "detail": image_url.detail.as_deref().unwrap_or("auto"),
                        }
                    })
                }
            })
            .collect();

        messages.push(serde_json::json!({
            "role": msg.role,
            "content": content_blocks,
        }));
    }

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "max_tokens": request.max_tokens,
    });

    let url = format!("{BASE_URL}{CHAT_ENDPOINT}");

    log::info!(
        "[vision] request: model={model}, task={}, images={}",
        request.task,
        request
            .messages
            .iter()
            .flat_map(|m| m.content.iter())
            .filter(|b| matches!(b, super::types::VisionContentBlock::ImageUrl { .. }))
            .count()
    );

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let kind = if e.is_timeout() { "timeout" } else { "network" };
            AppError::Ai {
                kind: kind.to_string(),
                message: e.to_string(),
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let kind = AiErrorKind::classify(Some(status.as_u16()), &body_text);
        return Err(AppError::Ai {
            kind: kind.to_string(),
            message: body_text,
        });
    }

    let json: Value = response.json().await.map_err(|e| AppError::Ai {
        kind: "invalid_response".into(),
        message: format!("响应 JSON 解析失败: {e}"),
    })?;

    let text = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let usage_total = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
    log::info!(
        "[vision] done: model={model}, text={}chars, tokens={usage_total}",
        text.len()
    );

    Ok(text)
}

/// 将本地图片文件编码为 base64 data URL
pub fn encode_image_to_data_url(path: &std::path::Path) -> Result<String, AppError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let bytes = std::fs::read(path).map_err(|e| AppError::Io(e))?;
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    let b64 = STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}
