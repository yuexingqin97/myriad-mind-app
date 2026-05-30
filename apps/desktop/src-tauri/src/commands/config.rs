// ============================================================
// 配置命令 — 配置读写 + OS 密钥链
// 与 docs/architecture.md §2 对齐
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFileInfo {
    pub path: String,
    pub exists: bool,
}

/// 获取配置文件默认路径
fn config_path() -> PathBuf {
    dirs_next().join("myriad-mind-config.json")
}

fn dirs_next() -> PathBuf {
    // 跨平台配置目录
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        dirs_macos()
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".config")
            })
    }
    .join("myriad-mind")
}

fn dirs_macos() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// 获取配置文件信息
#[tauri::command]
pub fn get_config_path() -> ConfigFileInfo {
    let path = config_path().join("config.json");
    ConfigFileInfo {
        path: path.to_string_lossy().to_string(),
        exists: path.exists(),
    }
}

/// 读取配置文件内容
#[tauri::command]
pub async fn read_config() -> Result<String, AppError> {
    let path = config_path().join("config.json");
    if path.exists() {
        Ok(std::fs::read_to_string(&path).map_err(AppError::Io)?)
    } else {
        // 返回空对象 JSON
        Ok("{}".to_string())
    }
}

/// 写入配置文件
#[tauri::command]
pub async fn write_config(content: String) -> Result<(), AppError> {
    let dir = config_path();
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;
    let path = dir.join("config.json");
    std::fs::write(&path, &content).map_err(AppError::Io)?;
    Ok(())
}

// ---- OS 密钥链操作 ----
//
// 各平台实现：
//   Windows: Credential Manager (wincred)
//   macOS:   Keychain (security framework)
//   Linux:   libsecret (secret-tool / keyring)
//
// 当前为 stub 实现，生产环境需要接入：
//   - Windows: windows-credentials crate
//   - macOS: security-framework crate
//   - Linux: oo7 or libsecret crate

#[derive(Debug, Serialize, Deserialize)]
pub struct KeychainEntry {
    pub service: String,
    pub account: String,
    pub exists: bool,
}

/// 检查密钥链中是否存在指定凭据
#[tauri::command]
pub async fn check_keychain_entry(service: String, account: String) -> Result<KeychainEntry, AppError> {
    // Stub: 生产环境需调用 OS 原生 API
    Ok(KeychainEntry {
        service,
        account,
        exists: false,
    })
}

/// 将凭据写入 OS 密钥链
#[tauri::command]
pub async fn store_keychain_entry(
    service: String,
    account: String,
    _secret: String,
) -> Result<(), AppError> {
    // Stub: 生产环境需调用 OS 原生 API
    let _ = (service, account);
    Err(AppError::Other(
        "密钥链功能尚未实现。请通过系统设置手动配置 API Key。".into(),
    ))
}

/// 从 OS 密钥链读取凭据
#[tauri::command]
pub async fn read_keychain_entry(
    service: String,
    account: String,
) -> Result<String, AppError> {
    // Stub: 生产环境需调用 OS 原生 API
    let _ = (service, account);
    Err(AppError::Other(
        "密钥链功能尚未实现。".into(),
    ))
}
