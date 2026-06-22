// ============================================================
// DeepSeekClient — OpenAI-compatible HTTP + SSE 流式解析
// ============================================================

use super::types::{
    AgentTurnResult, AiErrorKind, MindRequest, MindResponse, MindStreamEvent, ReasoningEffort,
    ThinkingConfig, TokenUsage, ToolCall,
};
use crate::error::AppError;
use reqwest::Client;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio_stream::StreamExt;

const BASE_URL: &str = "https://api.deepseek.com";
const CHAT_ENDPOINT: &str = "/chat/completions";

/// 选择模型：Pro vs Flash
///
/// 返回 (model_id, reason) —— reason 用于结构化日志（设计文档 §五「模型路由」埋点），
/// 便于事后 grep/过滤 agent 路由决策。
fn pick_model(request: &MindRequest) -> (&str, &'static str) {
    if let Some(ref m) = request.model_override {
        return (m.as_str(), "model_override");
    }
    match request.task {
        super::types::AiTask::Summary
        | super::types::AiTask::Translation
        | super::types::AiTask::NextStepSuggestion
        | super::types::AiTask::ResourceRecommend => {
            ("deepseek-v4-flash", "lightweight_task")
        }
        _ => ("deepseek-v4-pro", "default_heavy_task"),
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
    let (model, route_reason) = pick_model(request);
    let body = build_body(request, model);

    let url = format!("{BASE_URL}{CHAT_ENDPOINT}");

    // ---- LLM 请求埋点（设计文档 §五）----
    // 红线：绝不记 api_key / Authorization header / 完整 prompt 原文。
    //       prompt 只记摘要（前 200 字符）+ 长度；max_tokens 是预算而非已用量。
    let prompt_summary: String = request
        .messages
        .iter()
        .map(|m| m.content.chars().take(200).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ⏐ ")
        .chars().take(200).collect::<String>(); // 整体截断 200 字，防止多轮累积超标
    let prompt_chars: usize = request
        .messages
        .iter()
        .map(|m| m.content.chars().count())
        .sum();
    let max_tokens_budget = request.max_tokens.unwrap_or(65536);
    log::debug!(
        target: "agent",
        "[llm] phase=request model={model} route_reason={route_reason} task={} stream={} max_tokens_budget={max_tokens_budget} prompt_chars={prompt_chars} system_prompt_chars={} prompt_summary={:?}",
        request.task,
        request.stream,
        request.system_prompt.chars().count(),
        prompt_summary,
    );

    // 计时：覆盖整个 HTTP 请求 + 流式接收
    let req_start = std::time::Instant::now();

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
            log::warn!(
                target: "agent",
                "[llm] phase=request_failed model={model} kind={kind} duration_ms={} err={}",
                req_start.elapsed().as_millis(),
                e
            );
            AppError::Ai {
                kind: kind.to_string(),
                message: e.to_string(),
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let kind = AiErrorKind::classify(Some(status.as_u16()), &body_text);
        log::warn!(
            target: "agent",
            "[llm] phase=http_error model={model} status={} kind={} duration_ms={}",
            status.as_u16(),
            kind,
            req_start.elapsed().as_millis()
        );
        return Err(AppError::Ai {
            kind: kind.to_string(),
            // 红线：上游错误体可能回显请求片段，但这是 provider 返回而非我们注入的密钥，
            //       且需要保留给上层诊断。Authorization header 不在此 body 中。
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
                            input_tokens: u["input_tokens"]
                                .as_u64()
                                .or_else(|| u["prompt_tokens"].as_u64())
                                .unwrap_or(0) as u32,
                            output_tokens: u["output_tokens"]
                                .as_u64()
                                .or_else(|| u["completion_tokens"].as_u64())
                                .unwrap_or(0) as u32,
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

    // ---- LLM 响应埋点（设计文档 §五）----
    // finish_reason + token 用量 + 耗时；reasoning_content 单独分流（仅记长度，不记内容）。
    let duration_ms = req_start.elapsed().as_millis();
    let (in_tok, out_tok, reason_tok, total_tok) = usage
        .as_ref()
        .map(|u| (u.input_tokens, u.output_tokens, u.reasoning_tokens, u.total_tokens))
        .unwrap_or((0, 0, None, 0));
    let finish = finish_reason.as_deref().unwrap_or("none");
    let output_chars = full_text.len();
    let reasoning_chars = reasoning_text.len();
    let reasoning_shunted = !reasoning_text.is_empty();
    log::debug!(
        target: "agent",
        "[llm] phase=response model={model} finish_reason={finish} duration_ms={duration_ms} \
         input_tokens={in_tok} output_tokens={out_tok} reasoning_tokens={:?} total_tokens={total_tok} \
         output_chars={output_chars} reasoning_chars={reasoning_chars} reasoning_shunted={reasoning_shunted}",
        reason_tok,
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

/// 非流式 chat（带 tools）—— agent loop 用（设计文档 §6.1）。
///
/// 与 stream_deepseek 的区别：stream=false，解析 choices[0].message（含 tool_calls）。
/// 用非流式是因为 tool_use 的流式累积（delta.tool_calls 分片 JSON）复杂且本项目未验证；
/// agent 多轮决策用非流式更可靠，最终笔记内容由 runner 经 mind-stream 发出。
pub async fn chat_turn(
    _app_handle: &AppHandle,
    system_prompt: &str,
    messages: &[serde_json::Value],
    tools: &[serde_json::Value],
    thinking: Option<&ThinkingConfig>,
    api_key: &str,
) -> Result<AgentTurnResult, AppError> {
    let client = Client::new();
    let model = "deepseek-v4-pro";

    let mut all_messages = vec![serde_json::json!({ "role": "system", "content": system_prompt })];
    all_messages.extend_from_slice(messages);

    let mut body = serde_json::json!({
        "model": model,
        "messages": all_messages,
        "stream": false,
        "max_tokens": 131072,
    });
    if !tools.is_empty() {
        body["tools"] = serde_json::json!(tools);
    }
    if let Some(thinking) = thinking {
        if thinking.enabled {
            body["thinking"] = serde_json::json!({ "type": "enabled" });
            if let Some(effort) = &thinking.effort {
                body["reasoning_effort"] = match effort {
                    ReasoningEffort::High => serde_json::json!("high"),
                    ReasoningEffort::Max => serde_json::json!("max"),
                };
            }
        }
    }

    let prompt_chars: usize = all_messages
        .iter()
        .map(|m| {
            m.get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.chars().count())
                .unwrap_or(0)
        })
        .sum();
    log::debug!(
        target: "agent",
        "[llm] phase=chat_turn model={model} tools={} messages={} prompt_chars={prompt_chars}",
        tools.len(),
        all_messages.len()
    );

    let req_start = std::time::Instant::now();
    let response = client
        .post(format!("{BASE_URL}{CHAT_ENDPOINT}"))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Ai {
            kind: if e.is_timeout() { "timeout" } else { "network" }.into(),
            message: e.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let kind = AiErrorKind::classify(Some(status.as_u16()), &body_text);
        log::warn!(
            target: "agent",
            "[llm] phase=chat_turn_error status={} kind={} duration_ms={}",
            status.as_u16(),
            kind,
            req_start.elapsed().as_millis()
        );
        return Err(AppError::Ai {
            kind: kind.to_string(),
            message: body_text,
        });
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::Ai {
            kind: "invalid_response".into(),
            message: format!("响应 JSON 解析失败: {e}"),
        })?;

    let choice = &json["choices"][0];
    let message = choice["message"].clone();
    let finish_reason = choice["finish_reason"].as_str().map(String::from);
    let content = message["content"].as_str().map(String::from);
    let reasoning_content = message["reasoning_content"].as_str().map(String::from);

    let tool_calls: Vec<ToolCall> = message["tool_calls"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|t| serde_json::from_value::<ToolCall>(t.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let usage = json.get("usage").map(|u| TokenUsage {
        input_tokens: u["input_tokens"]
            .as_u64()
            .or_else(|| u["prompt_tokens"].as_u64())
            .unwrap_or(0) as u32,
        output_tokens: u["output_tokens"]
            .as_u64()
            .or_else(|| u["completion_tokens"].as_u64())
            .unwrap_or(0) as u32,
        reasoning_tokens: u["completion_tokens_details"]
            .get("reasoning_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
    });

    log::debug!(
        target: "agent",
        "[llm] phase=chat_turn_done finish={:?} tool_calls={} duration_ms={} total_tokens={}",
        finish_reason,
        tool_calls.len(),
        req_start.elapsed().as_millis(),
        usage.as_ref().map(|u| u.total_tokens).unwrap_or(0)
    );

    Ok(AgentTurnResult {
        message,
        content,
        tool_calls,
        reasoning_content,
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

    // ---- Vision LLM 请求埋点（设计文档 §五）----
    let image_count = request
        .messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter(|b| matches!(b, super::types::VisionContentBlock::ImageUrl { .. }))
        .count();
    let text_chars: usize = request
        .messages
        .iter()
        .flat_map(|m| m.content.iter())
        .map(|b| match b {
            super::types::VisionContentBlock::Text { text } => text.chars().count(),
            _ => 0,
        })
        .sum();
    log::debug!(
        target: "agent",
        "[vision] phase=request model={model} task={} images={image_count} text_chars={text_chars} max_tokens={:?}",
        request.task,
        request.max_tokens
    );

    let req_start = std::time::Instant::now();

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let kind = if e.is_timeout() { "timeout" } else { "network" };
            log::warn!(
                target: "agent",
                "[vision] phase=request_failed model={model} kind={kind} duration_ms={} err={}",
                req_start.elapsed().as_millis(),
                e
            );
            AppError::Ai {
                kind: kind.to_string(),
                message: e.to_string(),
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let kind = AiErrorKind::classify(Some(status.as_u16()), &body_text);
        log::warn!(
            target: "agent",
            "[vision] phase=http_error model={model} status={} kind={} duration_ms={}",
            status.as_u16(),
            kind,
            req_start.elapsed().as_millis()
        );
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
    log::debug!(
        target: "agent",
        "[vision] phase=response model={model} duration_ms={} output_chars={} total_tokens={usage_total}",
        req_start.elapsed().as_millis(),
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
