// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let config_dir = std::path::PathBuf::from(&home).join(".myriad-mind-app");

    // 确保配置目录存在（.env 加载等仍需此目录；日志初始化已移至 lib.rs 的 tauri-plugin-log）
    let _ = std::fs::create_dir_all(&config_dir);

    // 加载 .env
    let user_env = config_dir.join(".env");
    if user_env.exists() {
        let _ = dotenvy::from_path(&user_env);
    }
    myriad_mind_lib::run()
}
