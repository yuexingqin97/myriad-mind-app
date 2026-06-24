// ============================================================
// 大衍决桌面端 — Tauri 2.x 后端入口
// ============================================================

mod agent;
mod commands;
mod error;

use commands::{
    ai::{run_mind_task, test_deepseek_connection},
    ai_douyin::list_ai_douyin_tasks,
    config::{
        get_config_info, is_first_launch, read_config, reset_config, write_config,
    },
    deps::{
        detect_all_deps, detect_faster_whisper, detect_ffmpeg, detect_gpu, detect_python,
        detect_python_auto, detect_ytdlp,
    },
    fs::{cleanup_temp, copy_asset, get_cache_dir, open_cache_dir, pick_folder, read_text_file, scan_directory, write_note},
    logging::{open_log_dir, set_log_level},
    media::extract_keyframes,
    pipeline::{execute_pipeline},
    python::{
        check_python_env, download_video, download_youtube_subtitles,
        install_faster_whisper, transcribe_audio,
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
    // 初始化日志（tauri-plugin-log，替代旧的 simple_logging）
    //
    // 三 Target：
    //   - Stdout  → 终端 / devtools 实时看（dev 友好）
    //   - Folder  → ~/.myriad-mind-app/logs/myriad-mind.log（持久化、事后排查、按大小轮转）
    //   - Webview → Rust 日志转发到前端 console（F12 直接看 Rust 内部，Agent 调试利器）
    //
    // 级别：dev 构建 Trace（全量，含 tool use / loop 每轮）；release 构建 Info（用户级）。
    // 运行时可通过 set_log_level 命令动态调级（log::set_max_level）。
    let log_plugin = build_log_plugin();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(log_plugin)
        .invoke_handler(tauri::generate_handler![
            // 基础
            health_check,
            get_version,
            // 依赖检测
            detect_python,
            detect_python_auto,
            detect_ffmpeg,
            detect_faster_whisper,
            detect_ytdlp,
            detect_gpu,
            detect_all_deps,
            // 配置
            get_config_info,
            is_first_launch,
            read_config,
            write_config,
            reset_config,
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
            get_cache_dir,
            open_cache_dir,
            // 管线编排
            execute_pipeline,
            // AI (DeepSeek)
            run_mind_task,
            test_deepseek_connection,
            // 日志与调试
            set_log_level,
            open_log_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 构造 tauri-plugin-log 插件实例
///
/// 参考 cc-switch `src-tauri/src/lib.rs:303-338`，本项目在其基础上额外加 `Webview` target
/// （设计文档 §4.2 重点）。日志目录由 commands::logging::log_dir() 统一计算，
/// 与 open_log_dir 命令打开的路径严格一致。
fn build_log_plugin<R: tauri::Runtime>() -> impl tauri::plugin::Plugin<R> {
    use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

    let log_dir = commands::logging::log_dir();

    // 确保日志目录存在（tauri-plugin-log 不会自动建目录）
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        // 此处 log 系统尚未初始化，用 eprintln 兜底
        eprintln!("[logging] 创建日志目录失败: {e}");
    }

    // 启动时清掉旧日志文件（单文件覆盖效果，与 cc-switch 行为一致）
    let log_file_path = log_dir.join("myriad-mind.log");
    let _ = std::fs::remove_file(&log_file_path);

    // dev 构建 Trace（全量），release 构建 Info（用户级）
    let default_level = if cfg!(debug_assertions) {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    };

    tauri_plugin_log::Builder::default()
        .level(default_level)
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Folder {
                path: log_dir,
                file_name: Some("myriad-mind".into()),
            }),
            Target::new(TargetKind::Webview),
        ])
        // KeepSome(2) 是最小安全值（KeepSome(n) 内部 n-2 运算，n=1 会 usize 下溢）
        .rotation_strategy(RotationStrategy::KeepSome(2))
        // 开发 Trace 日志涨得快，50MB 比 cc-switch 的 1GB 更保守
        .max_file_size(50 * 1024 * 1024)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}
