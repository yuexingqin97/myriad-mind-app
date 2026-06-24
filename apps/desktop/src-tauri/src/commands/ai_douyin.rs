// ============================================================
// AI Douyin 任务查询 — Rust 直连（替代 list_ai_douyin_tasks.py）
//
// 原 Python 脚本用 urllib.request GET {base}/api/v1/tasks，
// 此处用 reqwest 复刻同一逻辑，省去子进程中转。
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};

/// 默认 AI Douyin API 地址（与原 Python 脚本 DEFAULT_API_BASE 一致）
const DEFAULT_API_BASE: &str = "https://ai-douyin.top9.cc";

/// AI Douyin 任务列表结果
///
/// `data` 通过 `#[serde(flatten)]` 捕获上游 API 返回的完整 JSON 对象，
/// 保持与原 Python 脚本 `--json` 输出完全一致的形状。
#[derive(Debug, Serialize, Deserialize)]
pub struct AiDouyinTaskList {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// 构造任务列表端点 URL
///
/// 复刻 Python `build_tasks_endpoint` 逻辑：
/// - trim 尾部 `/`
/// - 以 `/api/v1` 结尾 → 追加 `/tasks`
/// - 以 `/api` 结尾 → 追加 `/v1/tasks`
/// - 其他 → 追加 `/api/v1/tasks`
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

/// 查询 AI Douyin 任务列表（内部函数，供 Tauri 命令与 Agent 工具复用）
///
/// - `api_key`: 用户在设置页配置的 `ai_douyin_api_key`
/// - `api_base`: 可选自定义 base URL，None 时用默认 `https://ai-douyin.top9.cc`
/// - `page` / `page_size`: 分页参数
/// - `status` / `search`: 可选过滤
pub async fn fetch_ai_douyin_tasks(
    api_key: &str,
    api_base: Option<&str>,
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<&str>,
    search: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    let base = api_base.unwrap_or(DEFAULT_API_BASE);
    let endpoint = build_tasks_endpoint(base);

    log::debug!(target: "agent", "[douyin] phase=list_tasks endpoint={endpoint}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(AppError::Http)?;

    let mut req = client
        .get(&endpoint)
        .header("X-API-Key", api_key)
        .query(&[
            ("page", page.unwrap_or(1).to_string()),
            ("pageSize", page_size.unwrap_or(20).to_string()),
        ]);

    if let Some(s) = status {
        req = req.query(&[("status", s)]);
    }
    if let Some(q) = search {
        req = req.query(&[("search", q)]);
    }

    let resp = req.send().await.map_err(AppError::Http)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // 不回显完整 body（可能含会话标识），只取前 200 字符摘要
        let body_summary: String = body.chars().take(200).collect();
        log::warn!(
            target: "agent",
            "[douyin] phase=list_tasks_failed status={status} body_summary={body_summary:?}"
        );
        return Err(AppError::Other(format!(
            "AI Douyin 任务查询失败: HTTP {status} {body_summary}"
        )));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| {
        AppError::Other(format!("AI Douyin 任务响应解析失败: {e}"))
    })?;

    log::debug!(
        target: "agent",
        "[douyin] phase=list_tasks_done keys={:?}",
        json.as_object().map(|o| o.keys().collect::<Vec<_>>()).unwrap_or_default()
    );

    Ok(json)
}

/// 查询 AI Douyin 任务列表（Tauri 命令）
///
/// 替代原 `python.rs::list_ai_douyin_tasks`（Python 子进程中转）。
/// `python_path` 参数保留以维持前端 IPC 契约（`pythonPath`），但不再使用。
#[tauri::command]
pub async fn list_ai_douyin_tasks(
    #[allow(unused_variables)] python_path: String,
    api_key: String,
    api_base: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<String>,
    search: Option<String>,
) -> Result<AiDouyinTaskList, AppError> {
    let data = fetch_ai_douyin_tasks(
        &api_key,
        api_base.as_deref(),
        page,
        page_size,
        status.as_deref(),
        search.as_deref(),
    )
    .await?;

    Ok(AiDouyinTaskList { data })
}

#[cfg(test)]
mod tests {
    use super::build_tasks_endpoint;

    #[test]
    fn builds_default_endpoint() {
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
    }

    #[test]
    fn strips_trailing_slash() {
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc/"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
    }

    #[test]
    fn appends_tasks_to_api_v1() {
        assert_eq!(
            build_tasks_endpoint("https://example.com/api/v1"),
            "https://example.com/api/v1/tasks"
        );
    }

    #[test]
    fn appends_v1_tasks_to_api() {
        assert_eq!(
            build_tasks_endpoint("https://example.com/api"),
            "https://example.com/api/v1/tasks"
        );
    }
}
