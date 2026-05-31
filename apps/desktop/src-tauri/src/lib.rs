// ============================================================
// 大衍决桌面端 — Tauri 2.x 后端入口
// ============================================================

mod commands;
mod error;

use commands::{
    ai::{run_mind_task, test_deepseek_connection},
    config::{
        check_keychain_entry, get_config_info, is_first_launch, read_config,
        read_keychain_entry, reset_config, store_keychain_entry, write_config,
    },
    deps::{detect_all_deps, detect_ffmpeg, detect_gpu, detect_python, detect_python_auto, detect_ytdlp},
    fs::{cleanup_temp, copy_asset, pick_folder, read_text_file, scan_directory, write_note},
    pipeline::execute_pipeline,
    python::{
        check_python_env, download_video, download_youtube_subtitles, extract_keyframes,
        install_faster_whisper, list_ai_douyin_tasks, transcribe_audio,
    },
};

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
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // 基础
            health_check,
            get_version,
            // 依赖检测
            detect_python,
            detect_python_auto,
            detect_ffmpeg,
            detect_ytdlp,
            detect_gpu,
            detect_all_deps,
            // 配置
            get_config_info,
            is_first_launch,
            read_config,
            write_config,
            reset_config,
            check_keychain_entry,
            store_keychain_entry,
            read_keychain_entry,
            // Python 调度 (6 脚本)
            transcribe_audio,
            extract_keyframes,
            download_video,
            download_youtube_subtitles,
            install_faster_whisper,
            list_ai_douyin_tasks,
            check_python_env,
            // 文件系统
            pick_folder,
            scan_directory,
            read_text_file,
            write_note,
            cleanup_temp,
            copy_asset,
            // 管线编排
            execute_pipeline,
            // AI (DeepSeek)
            run_mind_task,
            test_deepseek_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
