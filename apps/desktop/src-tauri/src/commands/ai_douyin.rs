// ============================================================
// AI Douyin 任务查询 — Rust 直连（原 list_ai_douyin_tasks.py）
//
// 取代 Python 中转：用 reqwest 直连 AI Douyin /tasks 接口。
// 端点拼接 / query 构造 / X-API-Key 头 / 超时 / JSON 返回，
// 与原脚本逐字对齐（见《Python到Rust迁移计划》§6.4）。
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};

/// AI Douyin 任务列表结果（上游 API 返回的 JSON 对象，整体兜底为 Value）
#[derive(Debug, Serialize, Deserialize)]
pub struct AiDouyinTaskList {
    // API 返回的 JSON 结构，此处用 Value 兜底（与原 python.rs 行为一致）
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// 与脚本 list_ai_douyin_tasks.py::DEFAULT_API_BASE 一致
const DEFAULT_API_BASE: &str = "https://ai-douyin.top9.cc";

/// 与脚本 build_tasks_endpoint 一致：
/// 去尾斜杠后，按后缀拼接 /tasks、/v1/tasks 或 /api/v1/tasks。
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

/// 轻量 percent-encoding（与 Python urllib.parse.urlencode 对 query 值一致）。
/// 不引入新 crate，手写 query 值需要转义的字符子集（RFC 3986 unreserved 保留原样）。
fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
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
    let base = api_base
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_API_BASE);
    let endpoint = build_tasks_endpoint(base);

    // query 构造：page/pageSize 必带（默认 1/20，与脚本 argparse 默认一致），
    // status/search 非空才带（与脚本 fetch_tasks 一致）。
    let p = page.unwrap_or(1);
    let ps = page_size.unwrap_or(20);
    let mut url = format!("{endpoint}?page={p}&pageSize={ps}");
    if let Some(s) = status.as_deref() {
        if !s.is_empty() {
            url.push_str("&status=");
            url.push_str(&urlencode(s));
        }
    }
    if let Some(q) = search.as_deref() {
        if !q.is_empty() {
            url.push_str("&search=");
            url.push_str(&urlencode(q));
        }
    }

    log::debug!(target: "agent", "[ai_douyin] phase=list_tasks endpoint={endpoint}");
    let proc_start = std::time::Instant::now();

    // 30s 超时与脚本 urlopen(timeout=30) 一致。
    // 每次新建 client（低频查询，非热路径，与脚本每次新连接一致）。
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = client
        .get(&url)
        .header("X-API-Key", &api_key)
        .send()
        .await?;

    let status_code = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status_code.is_success() {
        // 与脚本 HTTPError 分支语义一致："HTTP {code} {body}"。
        // api_key 仅出现在请求头，body 为上游响应；按 §6.6 不视为高敏，原样入错误信息。
        log::warn!(
            target: "agent",
            "[ai_douyin] phase=failed status={status_code} duration_ms={}",
            proc_start.elapsed().as_millis()
        );
        return Err(AppError::Other(format!(
            "AI Douyin tasks request failed: HTTP {status_code} {body}"
        )));
    }

    // 与脚本一致：必须是 JSON 对象（dict）
    let data: serde_json::Value = serde_json::from_str(body.trim())
        .map_err(|e| AppError::Other(format!("AI Douyin tasks response is not valid JSON: {e}")))?;
    if !data.is_object() {
        return Err(AppError::Other(
            "AI Douyin tasks response is not a JSON object".into(),
        ));
    }

    log::debug!(
        target: "agent",
        "[ai_douyin] phase=done status={status_code} duration_ms={}",
        proc_start.elapsed().as_millis()
    );

    Ok(AiDouyinTaskList { data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_plain_base() {
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
    }

    #[test]
    fn endpoint_trailing_slash() {
        assert_eq!(
            build_tasks_endpoint("https://ai-douyin.top9.cc/"),
            "https://ai-douyin.top9.cc/api/v1/tasks"
        );
    }

    #[test]
    fn endpoint_api_v1_suffix() {
        assert_eq!(
            build_tasks_endpoint("https://example.com/api/v1"),
            "https://example.com/api/v1/tasks"
        );
    }

    #[test]
    fn endpoint_api_suffix() {
        assert_eq!(
            build_tasks_endpoint("https://example.com/api"),
            "https://example.com/api/v1/tasks"
        );
    }

    #[test]
    fn urlencode_spaces_and_cjk() {
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("中文"), "%E4%B8%AD%E6%96%87");
    }
}
