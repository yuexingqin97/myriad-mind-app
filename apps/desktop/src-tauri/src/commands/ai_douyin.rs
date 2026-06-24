// ============================================================
// AI Douyin API — Rust reqwest 直连，替代 list_ai_douyin_tasks.py
// ============================================================
// 旧方案：Python 脚本中转 (scripts/list_ai_douyin_tasks.py)
//          痛点：多一次子进程开销，api_key 通过 --api-key argv 传入可被进程列表泄漏
// 新方案：Rust reqwest 直连
//          收益：安全（api_key 仅在内存中）、日志清晰、省子进程开销
// ============================================================

use crate::error::AppError;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

/// AI Douyin 任务列表查询（Rust reqwest 直连实现）
///
/// 对应原 scripts/list_ai_douyin_tasks.py 的全部功能。
/// 与旧版 Tauri 命令签名兼容（去掉了不再需要的 python_path 参数）。
#[tauri::command]
pub async fn list_ai_douyin_tasks(
    api_key: String,
    api_base: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<String>,
    search: Option<String>,
) -> Result<Value, AppError> {
    let base = api_base.unwrap_or_else(|| "https://ai-douyin.top9.cc".to_string());
    let endpoint = build_tasks_endpoint(&base);
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(20);

    log::debug!(
        target: "agent",
        "[douyin] phase=list_tasks endpoint={endpoint} page={page} page_size={page_size} status={} search={}",
        status.as_deref().unwrap_or(""),
        search.as_deref().unwrap_or("")
    );

    let mut url = reqwest::Url::parse(&endpoint).map_err(|e| {
        AppError::Other(format!("AI Douyin 端点 URL 无效: {e}"))
    })?;

    {
        let mut q = url.query_pairs_mut();
        q.append_pair("page", &page.to_string());
        q.append_pair("pageSize", &page_size.to_string());
        if let Some(ref s) = status {
            if !s.is_empty() {
                q.append_pair("status", s);
            }
        }
        if let Some(ref s) = search {
            if !s.is_empty() {
                q.append_pair("search", s);
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(AppError::Http)?;

    let resp = client
        .get(url)
        .header("X-API-Key", &api_key)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Other(format!(
            "AI Douyin 任务查询失败: HTTP {status} {body}"
        )));
    }

    let json: Value = resp.json().await.map_err(|e| {
        AppError::Other(format!("AI Douyin 响应 JSON 解析失败: {e}"))
    })?;

    log::debug!(
        target: "agent",
        "[douyin] phase=list_tasks_done page={page} total={}",
        json.get("total").and_then(|v| v.as_u64()).unwrap_or(0)
    );

    Ok(json)
}

/// 构造 AI Douyin 任务列表 API 端点
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

// ============================================================
// 复用类型（供 download_video_candidates 的 API 查询部分使用）
// ============================================================

/// AI Douyin 视频解析响应（解析接口，非任务列表）
#[derive(Debug, Deserialize)]
pub struct AiDouyinResolveResponse {
    pub title: Option<String>,
    pub desc: Option<String>,
    #[serde(default)]
    pub candidates: Vec<AiDouyinVideoCandidate>,
}

#[derive(Debug, Deserialize)]
pub struct AiDouyinVideoCandidate {
    pub url: Option<String>,
    pub domain: Option<String>,
    pub format: Option<String>,
}

/// 通过 AI Douyin API 解析视频 URL，返回候选列表 JSON 文件路径
///
/// 对应原 download_video_candidates.py 的 API 查询部分。
/// 下载部分仍保留 yt-dlp（Python），此处只做解析。
pub async fn resolve_via_ai_douyin(
    video_url: &str,
    temp_dir: &std::path::Path,
) -> Result<std::path::PathBuf, AppError> {
    let api_key = crate::commands::config::read_config_value("ai_douyin_api_key").unwrap_or_default();
    let api_base = crate::commands::config::read_config_value("ai_douyin_api_base")
        .unwrap_or_else(|| "https://ai-douyin.top9.cc".to_string());

    if api_key.is_empty() {
        return Err(AppError::Config("AI Douyin API Key 未配置".into()));
    }

    let endpoint = build_resolve_endpoint(&api_base);

    log::debug!(
        target: "agent",
        "[douyin] phase=resolve endpoint={endpoint} url_len={}",
        video_url.len()
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(AppError::Http)?;

    let mut url = reqwest::Url::parse(&endpoint).map_err(|e| {
        AppError::Other(format!("AI Douyin 解析端点 URL 无效: {e}"))
    })?;
    url.query_pairs_mut().append_pair("url", video_url);

    let resp = client
        .get(url)
        .header("X-API-Key", &api_key)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Other(format!(
            "AI Douyin 视频解析失败: HTTP {status} {body}"
        )));
    }

    let json: Value = resp.json().await.map_err(|e| {
        AppError::Other(format!("AI Douyin 解析响应 JSON 解析失败: {e}"))
    })?;

    // 写入临时文件供后续下载步骤使用
    let json_path = temp_dir.join("ai_douyin_response.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&json)?)?;

    log::debug!(
        target: "agent",
        "[douyin] phase=resolved json_path={}",
        json_path.display()
    );

    Ok(json_path)
}

fn build_resolve_endpoint(api_base: &str) -> String {
    let trimmed = api_base.trim_end_matches('/');
    if trimmed.ends_with("/api/v1") {
        format!("{trimmed}/resolve")
    } else if trimmed.ends_with("/api") {
        format!("{trimmed}/v1/resolve")
    } else {
        format!("{trimmed}/api/v1/resolve")
    }
}
