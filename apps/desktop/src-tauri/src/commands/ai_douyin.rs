// ============================================================
// AI Douyin 任务查询 — Rust 直连 HTTP API，替代 list_ai_douyin_tasks.py
// ============================================================

use crate::commands::config::read_config_value;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

const DEFAULT_API_BASE: &str = "https://ai-douyin.top9.cc";

/// AI Douyin 任务列表结果（透传上游 API 返回的完整 JSON）
#[derive(Debug, Serialize, Deserialize)]
pub struct AiDouyinTaskList {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// 构建 AI Douyin API 请求 URL
fn build_tasks_endpoint(api_base: &str) -> String {
    let trimmed = api_base.trim_end_matches('/');
    if trimmed.ends_with("/api/v1") {
        format!("{trimmed}/tasks")
    } else if trimmed.ends_with("/api") {
        format!("{trimmed}/v1/tasks")
    } else {
        format!("{trimmed}/api/v1/tasks")
    }
}

/// 查询 AI Douyin 任务列表（Rust reqwest 直连，不再走 Python 子进程）
#[tauri::command]
pub async fn list_ai_douyin_tasks(
    api_key: String,
    api_base: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<String>,
    search: Option<String>,
) -> Result<AiDouyinTaskList, AppError> {
    let base = api_base
        .filter(|b| !b.is_empty())
        .or_else(|| read_config_value("ai_douyin_api_base"))
        .unwrap_or_else(|| DEFAULT_API_BASE.to_string());

    let endpoint = build_tasks_endpoint(&base);

    let page_val = page.unwrap_or(1);
    let page_size_val = page_size.unwrap_or(20);

    let mut query: Vec<(&str, String)> = vec![
        ("page", page_val.to_string()),
        ("pageSize", page_size_val.to_string()),
    ];
    if let Some(ref s) = status {
        if !s.is_empty() {
            query.push(("status", s.clone()));
        }
    }
    if let Some(ref q) = search {
        if !q.is_empty() {
            query.push(("search", q.clone()));
        }
    }

    let url = reqwest::Url::parse_with_params(&endpoint, &query)
        .map_err(|e| AppError::Other(format!("AI Douyin API URL 构造失败: {e}")))?;

    log::debug!(
        target: "agent",
        "[douyin] phase=request url={url}"
    );

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("X-API-Key", &api_key)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| {
            log::warn!(target: "agent", "[douyin] phase=http_error err={e}");
            AppError::Http(e)
        })?;

    let status_code = response.status();
    if !status_code.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::warn!(
            target: "agent",
            "[douyin] phase=http_error status={status_code} body_len={}",
            body.len()
        );
        return Err(AppError::Other(format!(
            "AI Douyin API 返回 HTTP {status_code}: {}",
            &body[..body.len().min(300)]
        )));
    }

    let payload: serde_json::Value = response.json().await.map_err(|e| {
        log::warn!(target: "agent", "[douyin] phase=parse_error err={e}");
        AppError::Http(e)
    })?;

    log::debug!(
        target: "agent",
        "[douyin] phase=done total={} page={}",
        payload.get("total").and_then(|v| v.as_u64()).unwrap_or(0),
        payload.get("page").and_then(|v| v.as_u64()).unwrap_or(1)
    );

    Ok(AiDouyinTaskList { data: payload })
}
