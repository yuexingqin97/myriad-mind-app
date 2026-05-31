// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 从 ~/.myriad-mind-app/.env 加载环境变量（开发/生产统一，不碰项目目录）
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let user_env = std::path::PathBuf::from(&home).join(".myriad-mind-app").join(".env");
    if user_env.exists() {
        let _ = dotenvy::from_path(&user_env);
    }
    myriad_mind_lib::run()
}
