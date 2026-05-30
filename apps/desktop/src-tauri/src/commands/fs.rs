// ============================================================
// 文件系统命令 — 笔记读写 / 目录扫描 / 临时文件清理
// ============================================================

use crate::error::AppError;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct DirScanResult {
    pub path: String,
    pub files: Vec<FileEntry>,
    pub total_count: usize,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub file_type: String, // "video" | "audio" | "text" | "other"
    pub size_bytes: u64,
}

/// 扫描目录下所有可处理文件
#[tauri::command]
pub async fn scan_directory(dir_path: String) -> Result<DirScanResult, AppError> {
    let path = PathBuf::from(&dir_path);
    if !path.is_dir() {
        return Err(AppError::Other(format!("路径不是目录: {dir_path}")));
    }

    let mut files = Vec::new();
    scan_recursive(&path, &mut files, 2) // 最多递归 2 层
        .map_err(AppError::Io)?;

    Ok(DirScanResult {
        path: dir_path,
        total_count: files.len(),
        files,
    })
}

fn scan_recursive(
    dir: &PathBuf,
    files: &mut Vec<FileEntry>,
    max_depth: usize,
) -> Result<(), std::io::Error> {
    if max_depth == 0 {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry
            .file_name()
            .to_string_lossy()
            .to_string();

        if path.is_dir() {
            // 跳过隐藏目录和常见忽略目录
            let skip = name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "__pycache__"
                || name == ".git";
            if !skip {
                scan_recursive(&path, files, max_depth - 1)?;
            }
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let file_type = classify_file_type(&ext);
            if file_type != "other" {
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                files.push(FileEntry {
                    name,
                    path: path.to_string_lossy().to_string(),
                    file_type: file_type.to_string(),
                    size_bytes,
                });
            }
        }
    }

    // 按类型排序
    files.sort_by_key(|f| f.file_type.clone());

    Ok(())
}

fn classify_file_type(ext: &str) -> &str {
    match ext {
        "mp4" | "mov" | "avi" | "mkv" | "webm" => "video",
        "mp3" | "wav" | "m4a" | "flac" | "ogg" | "aac" => "audio",
        "md" | "txt" | "pdf" | "html" | "htm" | "rst" | "org" => "text",
        _ => "other",
    }
}

/// 读取文本文件内容
#[tauri::command]
pub async fn read_text_file(file_path: String) -> Result<String, AppError> {
    let content = std::fs::read_to_string(&file_path).map_err(|e| {
        AppError::Other(format!("无法读取文件 {file_path}: {e}"))
    })?;
    Ok(content)
}

/// 写入 Markdown 笔记到指定路径
#[tauri::command]
pub async fn write_note(
    file_path: String,
    content: String,
) -> Result<(), AppError> {
    // 确保父目录存在
    if let Some(parent) = PathBuf::from(&file_path).parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
    }

    std::fs::write(&file_path, &content).map_err(AppError::Io)?;
    Ok(())
}

/// 清理临时文件目录
#[tauri::command]
pub async fn cleanup_temp(temp_dir: String) -> Result<(), AppError> {
    let path = PathBuf::from(&temp_dir);
    if path.exists() && path.starts_with(std::env::temp_dir()) {
        std::fs::remove_dir_all(&path).map_err(AppError::Io)?;
    }
    Ok(())
}

/// 复制截图到笔记 assets 目录
#[tauri::command]
pub async fn copy_asset(
    source: String,
    dest_dir: String,
) -> Result<String, AppError> {
    let src_path = PathBuf::from(&source);
    let dest_dir_path = PathBuf::from(&dest_dir);

    std::fs::create_dir_all(&dest_dir_path).map_err(AppError::Io)?;

    let filename = src_path
        .file_name()
        .ok_or_else(|| AppError::Other("无效文件名".into()))?;

    let dest_path = dest_dir_path.join(filename);
    std::fs::copy(&src_path, &dest_path).map_err(AppError::Io)?;

    Ok(dest_path.to_string_lossy().to_string())
}
