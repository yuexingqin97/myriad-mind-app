// ============================================================
// 配置命令 — 配置读写 + 首启检测 + OS 密钥链
// 配置目录: ~/.myriad-mind-app/ (所有平台统一)
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

/// 读取配置（无文件时返回空对象）
#[tauri::command]
pub async fn read_config() -> Result<String, AppError> {
    let path = config_file();
    if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| {
            AppError::Config(format!("读取配置失败: {e}"))
        })
    } else {
        Ok("{}".to_string())
    }
}

/// 写入配置（原子写入：先写 .tmp 再 rename）
#[tauri::command]
pub async fn write_config(content: String) -> Result<(), AppError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| {
        AppError::Config(format!("创建配置目录失败: {e}"))
    })?;

    let path = config_file();
    let tmp = dir.join("config.json.tmp");

    // 原子写入
    std::fs::write(&tmp, &content).map_err(|e| {
        AppError::Config(format!("写入配置失败: {e}"))
    })?;

    std::fs::rename(&tmp, &path).map_err(|e| {
        AppError::Config(format!("保存配置失败: {e}"))
    })?;

    Ok(())
}

/// 删除配置文件（用于重置）
#[tauri::command]
pub async fn reset_config() -> Result<(), AppError> {
    let path = config_file();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| {
            AppError::Config(format!("删除配置失败: {e}"))
        })?;
    }
    Ok(())
}

// ---- OS 密钥链 (Windows Credential Manager) ----

#[derive(Debug, Serialize, Deserialize)]
pub struct KeychainEntry {
    pub service: String,
    pub account: String,
    pub exists: bool,
}

/// 检查密钥链条目
#[tauri::command]
pub async fn check_keychain_entry(
    service: String,
    account: String,
) -> Result<KeychainEntry, AppError> {
    // Windows: Credential Manager
    #[cfg(target_os = "windows")]
    {
        match windows_credentials(&service, &account) {
            Ok(Some(_)) => Ok(KeychainEntry {
                service,
                account,
                exists: true,
            }),
            Ok(None) => Ok(KeychainEntry {
                service,
                account,
                exists: false,
            }),
            Err(e) => Err(AppError::Config(format!("密钥链读取失败: {e}"))),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (service, account);
        Ok(KeychainEntry {
            service: "stub".into(),
            account: "stub".into(),
            exists: false,
        })
    }
}

/// 写入凭据到 OS 密钥链
#[tauri::command]
pub async fn store_keychain_entry(
    service: String,
    account: String,
    secret: String,
) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        windows_credentials_write(&service, &account, &secret)
            .map_err(|e| AppError::Config(format!("密钥链写入失败: {e}")))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (service, account, secret);
        Err(AppError::Other("密钥链仅支持 Windows (v1)。".into()))
    }
}

/// 从 OS 密钥链读取凭据
#[tauri::command]
pub async fn read_keychain_entry(
    service: String,
    account: String,
) -> Result<String, AppError> {
    #[cfg(target_os = "windows")]
    {
        windows_credentials(&service, &account)?.ok_or_else(|| {
            AppError::Config(format!("密钥链中未找到: {service}/{account}"))
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (service, account);
        Err(AppError::Other("密钥链仅支持 Windows (v1)。".into()))
    }
}

/// 公开凭据读取接口 — 供 ai/engine.rs 等模块调用
#[cfg(target_os = "windows")]
pub fn cred_read(service: &str) -> Result<Option<String>, String> {
    let target = format!("myriad-mind/{service}");
    windows_cred::read(&target)
}

// ---- Windows Credential Manager ----
//
// 使用 Win32 CredReadW / CredWriteW API 读写 Windows 凭据管理器。
// 条目命名: myriad-mind/{service}  (如 myriad-mind/deepseek-api-key)

#[cfg(target_os = "windows")]
mod windows_cred {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Security::Credentials::{
        CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    /// 读取凭据，返回 UTF-8 密码；不存在返回 None
    pub fn read(target: &str) -> Result<Option<String>, String> {
        let target_h = HSTRING::from(target);

        let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();

        let result =
            unsafe { CredReadW(&target_h, CRED_TYPE_GENERIC, None, &mut cred_ptr) };

        if result.is_err() || cred_ptr.is_null() {
            return Ok(None);
        }

        unsafe {
            let cred = &*cred_ptr;
            let blob_ptr = cred.CredentialBlob;
            let blob_size = cred.CredentialBlobSize as usize;

            let secret = if blob_size > 0 && !blob_ptr.is_null() {
                let bytes = std::slice::from_raw_parts(blob_ptr, blob_size);
                let utf16: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16(&utf16).unwrap_or_default()
            } else {
                String::new()
            };

            CredFree(cred_ptr as *const _);

            Ok(Some(secret.trim_end_matches('\0').to_string()))
        }
    }

    /// 写入凭据
    pub fn write(
        target: &str,
        username: &str,
        secret: &str,
    ) -> Result<(), String> {
        let target_wide: Vec<u16> =
            target.encode_utf16().chain(std::iter::once(0)).collect();
        let username_wide: Vec<u16> =
            username.encode_utf16().chain(std::iter::once(0)).collect();
        let secret_utf16: Vec<u16> = secret.encode_utf16().collect();
        let blob_bytes: Vec<u8> =
            secret_utf16.iter().flat_map(|c| c.to_le_bytes()).collect();

        use windows::Win32::Security::Credentials::CRED_FLAGS;

        let credential = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_wide.as_ptr() as *mut _),
            Comment: PWSTR::null(),
            LastWritten: windows::Win32::Foundation::FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            },
            CredentialBlobSize: blob_bytes.len() as u32,
            CredentialBlob: blob_bytes.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR(username_wide.as_ptr() as *mut _),
        };

        unsafe {
            CredWriteW(&credential, 0)
                .map_err(|e| format!("CredWriteW 失败: {e:?}"))?;
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn windows_credentials(
    service: &str,
    _account: &str,
) -> Result<Option<String>, String> {
    let target = format!("myriad-mind/{service}");
    windows_cred::read(&target)
}

#[cfg(target_os = "windows")]
fn windows_credentials_write(
    service: &str,
    account: &str,
    secret: &str,
) -> Result<(), String> {
    let target = format!("myriad-mind/{service}");
    windows_cred::write(&target, account, secret)
}
