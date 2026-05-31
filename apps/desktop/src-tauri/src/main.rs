// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let config_dir = std::path::PathBuf::from(&home).join(".myriad-mind-app");

    // 确保配置目录存在
    let _ = std::fs::create_dir_all(&config_dir);

    // 初始化日志文件 ~/.myriad-mind-app/app.log
    let log_path = config_dir.join("app.log");
    let _ = simple_logging::log_to_file(log_path, log::LevelFilter::Debug);
    log::info!("大衍决启动 — 日志写入 {}", config_dir.display());

    // 加载 .env
    let user_env = config_dir.join(".env");
    if user_env.exists() {
        let _ = dotenvy::from_path(&user_env);
    }
    myriad_mind_lib::run()
}
