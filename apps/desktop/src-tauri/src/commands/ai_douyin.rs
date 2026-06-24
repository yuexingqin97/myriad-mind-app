// ============================================================
// AI Douyin HTTP 直连
// 原 list_ai_douyin_tasks.py 的纯中转逻辑迁到 Rust，直接走 reqwest。
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DEFAULT_API_BASE: &str = "https://ai-douyin.top9.cc";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// AI Douyin 任务列表结果
/// 保持与原 Python 脚本输出一致：整体是一个 JSON 对象，这里用 Value 兜底。
#[derive(Debug, Serialize, Deserialize)]
pub struct AiDouyinTaskList {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// 构建 tasks endpoint，规则与原 Python 脚本完全一致：
/// - 以 `/api/v1` 结尾 → 直接加 `/tasks`
/// - 以 `/api` 结尾 → 加 `/v1/tasks`
/// - 其它 → 加 `/api/v1/tasks`
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

/// 查询 AI Douyin 任务列表（Rust 直连，无 Python 中转）
#[tauri::command]
pub async fn list_ai_douyin_tasks(
    api_key: String,
    api_base: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<String>,
    search: Option<String>,
) -> Result<AiDouyinTaskList, AppError> {
    if api_key.is_empty() {
        return Err(AppError::Config("缺少 ai_douyin_api_key".into()));
    }

    let base = api_base.as_deref().unwrap_or(DEFAULT_API_BASE);
    let endpoint = build_tasks_endpoint(base);
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(20);

    let mut query: Vec<(&str, String)> = vec![
        ("page", page.to_string()),
        ("pageSize", page_size.to_string()),
    ];
    if let Some(s) = status {
        if !s.is_empty() {
            query.push(("status", s));
        }
    }
    if let Some(s) = search {
        if !s.is_empty() {
            query.push(("search", s));
        }
    }

    log::debug!(
        target: "agent",
        "[douyin] phase=list_tasks endpoint={endpoint} page={page} page_size={page_size}"
    );

    let client = reqwest::Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .build()
        .map_err(|e| AppError::Other(format!("创建 HTTP 客户端失败: {e}")))?;

    let response = client
        .get(&endpoint)
        .query(&query)
        .header("X-API-Key", &api_key)
        .send()
        .await?;

    let status_code = response.status();
    if !status_code.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Other(format!(
            "AI Douyin 查询失败: HTTP {status_code} {body}"
        )));
    }

    let data = response.json().await?;
    Ok(AiDouyinTaskList { data })
}

#[cfg(test)]
mod tests {
    use super::build_tasks_endpoint;

    #[test]
    fn endpoint_building() {
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc/"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc/api"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc/api/v1"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc/api/v1/"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
    }
}
