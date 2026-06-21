// ============================================================
// 日志与调试脚手架 — 运行时调级 + 打开日志目录
// ============================================================
//
// 配套 tauri-plugin-log（在 lib.rs run() 中注册）：
// - set_log_level：前端"日志级别"下拉即时下发，调 log::set_max_level 动态生效
// - open_log_dir：前端"打开日志目录"按钮，复用 ~/.myriad-mind-app/logs/ 目录
//
// 红线：日志绝不记录 API Key / Authorization。本模块不接触密钥。

use std::path::PathBuf;
use tauri::AppHandle;

use crate::error::AppError;

/// 解析用户配置目录 ~/.myriad-mind-app/（与 main.rs 的拼法保持一致）
///
/// 走 USERPROFILE（Windows）/ HOME（Unix）环境变量，失败则回退到 DirBuilder 兜底。
fn config_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(&home).join(".myriad-mind-app")
}

/// 日志目录 ~/.myriad-mind-app/logs/（与 lib.rs 中 tauri-plugin-log Folder target 一致）
pub fn log_dir() -> PathBuf {
    config_dir().join("logs")
}

/// 把字符串级别映射为 log::LevelFilter（不区分大小写）
///
/// 合法值：trace / debug / info / warn / error；非法值返回 None（由调用方报错）。
fn parse_level_filter(level: &str) -> Option<log::LevelFilter> {
    match level.to_ascii_lowercase().as_str() {
        "trace" => Some(log::LevelFilter::Trace),
        "debug" => Some(log::LevelFilter::Debug),
        "info" => Some(log::LevelFilter::Info),
        "warn" => Some(log::LevelFilter::Warn),
        "error" => Some(log::LevelFilter::Error),
        _ => None,
    }
}

/// Tauri 命令：运行时切换日志级别（即时生效）
///
/// 调用 log::set_max_level，影响 tauri-plugin-log 所有 Target（Stdout/Folder/Webview）。
/// 级别不落 config.json，由前端 localStorage 持久化（与 theme 同款"运行时偏好"语义）。
#[tauri::command]
pub fn set_log_level(level: String) -> Result<(), AppError> {
    let filter = parse_level_filter(&level)
        .ok_or_else(|| AppError::Other(format!("非法日志级别: {level}（期望 trace/debug/info/warn/error）")))?;
    log::set_max_level(filter);
    // 注意：不记 level 值以外的任何内容；不记调用方信息
    log::debug!(target: "agent", "[logging] level={level} source=frontend");
    Ok(())
}

/// Tauri 命令：在系统文件管理器中打开日志目录
///
/// 目录固定为 ~/.myriad-mind-app/logs/（tauri-plugin-log Folder target 写入位置）。
/// 实现沿用现有 open_cache_dir 的手写分平台 Command 模式（最省改动、不破坏现有逻辑）。
/// 不直接调用 tauri-plugin-opener，以保持与 open_cache_dir 一致的风格。
#[tauri::command]
pub fn open_log_dir(_app: AppHandle) -> Result<(), AppError> {
    let dir = log_dir();
    // 确保目录存在（即使尚无日志写入也允许打开）
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;

    let path_str = dir.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path_str)
            .spawn()
            .map_err(|e| AppError::Other(format!("无法打开日志目录: {e}")))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| AppError::Other(format!("无法打开日志目录: {e}")))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| AppError::Other(format!("无法打开日志目录: {e}")))?;
    }
    Ok(())
}
