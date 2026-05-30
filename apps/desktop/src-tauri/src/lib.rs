// ============================================================
// 大衍决桌面端 — Tauri 2.x 后端入口
// ============================================================

/// Tauri 命令：健康检查 / 依赖状态
#[tauri::command]
fn health_check() -> &'static str {
    "ok"
}

/// Tauri 命令：获取版本信息
#[tauri::command]
fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![health_check, get_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
