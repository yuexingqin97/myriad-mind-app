// ============================================================
// 配置命令 — 配置读写 + 首启检测
// 配置目录: ~/.myriad-mind-app/ (所有平台统一)
// API Key 等敏感字段也存 config.json（用户自行保管，不入 git）
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---- 路径 ----

/// 用户主目录
fn home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").unwrap_or_else(|_| ".".into()).into()
    }
}

/// 配置目录: ~/.myriad-mind-app/
pub fn config_dir() -> PathBuf {
    home_dir().join(".myriad-mind-app")
}

/// 配置文件完整路径
fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

/// 敏感字段（API Key）— 全量写配置时，新值为空则保留磁盘已有值，
/// 防止前端 config state 未同步把已配置的 key 覆盖成空。
const SECRET_FIELDS: &[&str] = &["deepseek_api_key", "ai_douyin_api_key"];

// ---- 数据结构 ----

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFileInfo {
    pub path: String,
    pub exists: bool,
    pub is_first_launch: bool,
}

// ---- 命令 ----

/// 获取配置文件路径信息（用于首启检测）
#[tauri::command]
pub fn get_config_info() -> ConfigFileInfo {
    let path = config_file();
    ConfigFileInfo {
        exists: path.exists(),
        is_first_launch: !path.exists(),
        path: path.to_string_lossy().to_string(),
    }
}

/// 检查是否首次启动
#[tauri::command]
pub fn is_first_launch() -> bool {
    !config_file().exists()
}

/// 从配置文件中读取一个字符串字段（Rust 内部用 — engine.rs/pipeline.rs 读 API Key）
pub fn read_config_value(key: &str) -> Option<String> {
    let path = config_file();
    let raw = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    json.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// 读取单个字段（前端 Tauri 命令）
#[tauri::command]
pub async fn get_config_value(key: String) -> Result<Option<String>, AppError> {
    Ok(read_config_value(&key))
}

/// 写入单个字段（读改写，原子写入）— 前端保存 API Key 用
#[tauri::command]
pub async fn set_config_value(key: String, value: String) -> Result<(), AppError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Config(format!("创建配置目录失败: {e}")))?;

    let path = config_file();
    let mut json: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    // 顶层必须是对象
    if !json.is_object() {
        json = serde_json::json!({});
    }
    json.as_object_mut()
        .unwrap()
        .insert(key, serde_json::Value::String(value));

    let tmp = dir.join("config.json.tmp");
    let content =
        serde_json::to_string_pretty(&json).map_err(|e| AppError::Config(format!("序列化失败: {e}")))?;
    std::fs::write(&tmp, &content).map_err(|e| AppError::Config(format!("写入配置失败: {e}")))?;
    std::fs::rename(&tmp, &path).map_err(|e| AppError::Config(format!("保存配置失败: {e}")))?;
    Ok(())
}

/// 读取配置（无文件时返回空对象）
#[tauri::command]
pub async fn read_config() -> Result<String, AppError> {
    let path = config_file();
    if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| AppError::Config(format!("读取配置失败: {e}")))
    } else {
        Ok("{}".to_string())
    }
}

/// 写入配置（原子写入：先写 .tmp 再 rename）
/// 全量写入时对 SECRET_FIELDS 做保护：新值为空则保留磁盘值
#[tauri::command]
pub async fn write_config(content: String) -> Result<(), AppError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Config(format!("创建配置目录失败: {e}")))?;

    let path = config_file();
    let tmp = dir.join("config.json.tmp");

    let final_content = protect_secret_fields(&path, &content);

    // 原子写入
    std::fs::write(&tmp, &final_content).map_err(|e| AppError::Config(format!("写入配置失败: {e}")))?;
    std::fs::rename(&tmp, &path).map_err(|e| AppError::Config(format!("保存配置失败: {e}")))?;

    Ok(())
}

/// 全量写配置前保护 API Key：新值为空时回填磁盘已有值，避免覆盖
fn protect_secret_fields(path: &std::path::Path, new_content: &str) -> String {
    let mut new_json: serde_json::Value = match serde_json::from_str(new_content) {
        Ok(v) => v,
        // 解析失败，不保护，原样写回（让上层报错或保留原行为）
        Err(_) => return new_content.to_string(),
    };

    let old_json: serde_json::Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    let (Some(new_obj), Some(old_obj)) = (new_json.as_object_mut(), old_json.as_object()) else {
        return serde_json::to_string_pretty(&new_json).unwrap_or_else(|_| new_content.to_string());
    };

    for field in SECRET_FIELDS {
        let new_empty = new_obj
            .get(*field)
            .and_then(|v| v.as_str())
            .map_or(true, |s| s.is_empty());
        if new_empty {
            if let Some(old_val) = old_obj.get(*field).and_then(|v| v.as_str()) {
                if !old_val.is_empty() {
                    new_obj.insert(
                        (*field).to_string(),
                        serde_json::Value::String(old_val.to_string()),
                    );
                }
            }
        }
    }

    serde_json::to_string_pretty(&new_json).unwrap_or_else(|_| new_content.to_string())
}

/// 删除配置文件（用于重置）
#[tauri::command]
pub async fn reset_config() -> Result<(), AppError> {
    let path = config_file();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| AppError::Config(format!("删除配置失败: {e}")))?;
    }
    Ok(())
}
