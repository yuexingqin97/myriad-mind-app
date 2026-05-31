// ============================================================
// Claude API 客户端 — SSE 流式笔记生成
// 与 docs/架构设计.md §2.5 对齐
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio_stream::StreamExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: String,
}

/// SSE 推送到前端的 payload
#[derive(Debug, Clone, Serialize)]
struct StreamDeltaPayload {
    delta: String,
}

/// 流式调用 Claude API 生成笔记
///
/// 每个 text_delta 通过 Tauri event "claude-stream-delta" 实时推送到前端
#[tauri::command]
pub async fn stream_note_generation(
    app_handle: AppHandle,
    messages: Vec<ClaudeMessage>,
    system_prompt: String,
    api_key: String,
) -> Result<String, AppError> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 8192,
        "messages": messages,
        "system": system_prompt,
        "stream": true,
    });

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "prompt-caching-2024-07-31")
        .json(&body)
        .send()
        .await
        .map_err(AppError::Http)?;

    if !response.status().is_success() {
        return Err(AppError::ClaudeApi {
            code: response.status().as_u16(),
            message: response.text().await.unwrap_or_default(),
        });
    }

    let mut stream = response.bytes_stream();
    let mut full_text = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            AppError::StreamError(format!("流读取失败: {e}"))
        })?;

        // 解析 SSE: "data: {...}"
        for line in chunk.split(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(line);
            if let Some(data) = line.strip_prefix("data: ") {
                // 跳过 [DONE]
                if data.trim() == "[DONE]" {
                    continue;
                }

                // 解析 JSON 提取 text_delta
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(text) = parsed["delta"]["text"].as_str() {
                        full_text.push_str(text);

                        // 推送增量到前端
                        let _ = app_handle.emit(
                            "claude-stream-delta",
                            StreamDeltaPayload {
                                delta: text.to_string(),
                            },
                        );
                    }

                    // content_block_delta 的 text
                    if let Some(delta_type) = parsed["type"].as_str() {
                        if delta_type == "content_block_delta" {
                            if let Some(text) = parsed["delta"]["text"].as_str() {
                                full_text.push_str(text);
                                let _ = app_handle.emit(
                                    "claude-stream-delta",
                                    StreamDeltaPayload {
                                        delta: text.to_string(),
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(full_text)
}

/// 非流式 Claude API 调用（用于简短请求如摘要、翻译）
#[tauri::command]
pub async fn call_claude(
    messages: Vec<ClaudeMessage>,
    system_prompt: String,
    api_key: String,
    max_tokens: Option<u32>,
) -> Result<String, AppError> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": max_tokens.unwrap_or(4096),
        "messages": messages,
        "system": system_prompt,
    });

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(AppError::Http)?;

    if !response.status().is_success() {
        let code = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::ClaudeApi {
            code,
            message: text,
        });
    }

    let json: serde_json::Value =
        response.json().await.map_err(AppError::Http)?;

    // 提取 content[0].text
    let text = json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(text)
}
