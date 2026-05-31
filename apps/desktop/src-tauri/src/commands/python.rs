// ============================================================
// Python 脚本调度 — 统一子进程调用模式
// 封装全部 6 个上游脚本为类型安全的 Rust 调用
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::process::Stdio;
use tokio::process::Command;

const PYTHON_MIN_VERSION: (u32, u32) = (3, 9);

// ============================================================
// 脚本路径解析
// ============================================================

/// 获取 scripts/ 目录的绝对路径
///
/// 开发模式: 从 CWD (apps/desktop/src-tauri/) 向上找到项目根目录的 scripts/
/// 生产模式: 从可执行文件同级目录查找 scripts/ (Tauri resources)
fn scripts_dir() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(found) = find_scripts_dir_from(&cwd) {
            return found;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            if let Some(found) = find_scripts_dir_from(exe_dir) {
                return found;
            }
        }
    }

    // 策略 1: 开发模式 — 从当前工作目录向上找项目根
    if let Ok(cwd) = std::env::current_dir() {
        // Tauri dev 的 CWD 通常是 apps/desktop/src-tauri/
        // 向上 2 级到项目根，再拼 scripts/
        if let Some(grandparent) = cwd.parent().and_then(|p| p.parent()) {
            let candidate = grandparent.join("scripts");
            if candidate.join("transcribe_faster_whisper.py").exists() {
                return candidate;
            }
        }
        // 也尝试 CWD 本身 (如果直接在项目根运行)
        let candidate = cwd.join("scripts");
        if candidate.join("transcribe_faster_whisper.py").exists() {
            return candidate;
        }
    }

    // 策略 2: 从可执行文件所在目录查找 (生产构建)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join("scripts");
            if candidate.join("transcribe_faster_whisper.py").exists() {
                return candidate;
            }
            // Windows: 可能是 Resources/scripts/ 子目录
            let candidate = exe_dir.join("Resources").join("scripts");
            if candidate.join("transcribe_faster_whisper.py").exists() {
                return candidate;
            }
        }
    }

    // 兜底: 相对路径 (让后续调用报出清晰的错误)
    PathBuf::from("scripts")
}

/// 获取脚本绝对路径
fn find_scripts_dir_from(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        for candidate in [
            dir.join("scripts"),
            dir.join("Resources").join("scripts"),
            dir.join("resources").join("scripts"),
        ] {
            if candidate.join("transcribe_faster_whisper.py").exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn script_path(name: &str) -> PathBuf {
    scripts_dir().join(name)
}

// ============================================================
// 通用脚本执行
// ============================================================

/// 通用 Python 脚本执行结果
#[derive(Debug, Serialize, Deserialize)]
pub struct PythonScriptResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// 执行 Python 脚本 — 通用模式
///
/// 统一模式: `python <script> [args...]` → 检查 exit code → 返回 stdout/stderr
pub async fn run_python_script(
    python_path: &str,
    script_name: &str,
    args: &[String],
) -> Result<PythonScriptResult, AppError> {
    let script = script_path(script_name);
    if !script.exists() {
        return Err(AppError::MissingDependency(format!(
            "脚本不存在: {} (查找路径: {})",
            script_name,
            script.display()
        )));
    }

    let (program, prefix_args) = python_command_parts(python_path);
    let output = Command::new(program)
        .args(prefix_args)
        .arg(&script)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            AppError::MissingDependency(format!("无法运行 Python '{}': {e}", python_path))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    let success = output.status.success();

    Ok(PythonScriptResult {
        success,
        stdout,
        stderr,
        exit_code,
    })
}

/// 执行脚本并解析 JSON stdout 为指定类型
async fn run_and_parse<T: serde::de::DeserializeOwned>(
    python_path: &str,
    script_name: &str,
    args: &[String],
    label: &str,
) -> Result<T, AppError> {
    let result = run_python_script(python_path, script_name, args).await?;

    if !result.success {
        return Err(AppError::PythonScript {
            script: script_name.to_string(),
            stderr: result.stderr,
        });
    }

    serde_json::from_str::<T>(result.stdout.trim()).map_err(|e| {
        AppError::Other(format!(
            "解析 {} 结果 JSON 失败: {e}\nstdout: {}",
            label,
            &result.stdout[..result.stdout.len().min(500)]
        ))
    })
}

// ============================================================
// 6 个脚本的类型化封装
// ============================================================

// ---- 1. transcribe_faster_whisper.py ----

/// ASR 转写结果
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub runtime: TranscriptionRuntime,
    pub result: TranscriptionData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionRuntime {
    pub model_size: String,
    pub device: String,
    pub compute_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionData {
    pub model_size: String,
    pub device: String,
    pub compute_type: String,
    pub language: String,
    pub language_probability: f64,
    pub segment_count: usize,
    pub audio_path: String,
    pub output_dir: String,
    pub srt_path: String,
    pub text_path: String,
}

/// 执行音频转写 (faster-whisper)
#[tauri::command]
pub async fn transcribe_audio(
    audio_path: String,
    output_dir: String,
    python_path: String,
    model_size: String,
    device: String,
) -> Result<TranscriptionResult, AppError> {
    run_and_parse(
        &python_path,
        "transcribe_faster_whisper.py",
        &[
            audio_path,
            "--output-dir".into(),
            output_dir,
            "--model-size".into(),
            model_size,
            "--device".into(),
            device,
        ],
        "ASR 转写",
    )
    .await
}

// ---- 2. extract_keyframes.py ----

/// 关键帧提取结果
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeResult {
    pub result: KeyframeData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeData {
    pub video_path: String,
    pub output_dir: String,
    pub mode: String,
    pub interval: u32,
    pub max_frames: u32,
    pub keyframes: Vec<KeyframeInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeInfo {
    pub file: String,
    pub timestamp_seconds: f64,
    pub timestamp_label: String,
}

/// 执行关键帧提取
#[tauri::command]
pub async fn extract_keyframes(
    video_path: String,
    output_dir: String,
    python_path: String,
    interval: u32,
    max_frames: u32,
    mode: String,
) -> Result<KeyframeResult, AppError> {
    run_and_parse(
        &python_path,
        "extract_keyframes.py",
        &[
            "--video".into(),
            video_path,
            "--output-dir".into(),
            output_dir,
            "--interval".into(),
            interval.to_string(),
            "--max-frames".into(),
            max_frames.to_string(),
            "--mode".into(),
            mode,
        ],
        "关键帧提取",
    )
    .await
}

// ---- 3. download_video_candidates.py ----

/// 视频下载结果
#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadVideoResult {
    pub video_path: String,
    pub selected_url_index: usize,
    pub selected_domain: String,
}

/// 执行视频下载（从 JSON 响应中选择最佳 URL 下载）
#[tauri::command]
pub async fn download_video(
    response_json_path: String,
    output_path: String,
    python_path: String,
    timeout: Option<u32>,
) -> Result<DownloadVideoResult, AppError> {
    let mut args = vec![
        "--response-json".into(),
        response_json_path,
        "--output".into(),
        output_path,
    ];
    if let Some(t) = timeout {
        args.push("--timeout".into());
        args.push(t.to_string());
    }

    run_and_parse(
        &python_path,
        "download_video_candidates.py",
        &args,
        "视频下载",
    )
    .await
}

// ---- 4. download_youtube_subtitles.py ----

/// YouTube 字幕下载结果
#[derive(Debug, Serialize, Deserialize)]
pub struct SubtitleResult {
    pub result: SubtitleData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubtitleData {
    pub url: String,
    pub languages: Vec<String>,
    pub vtt_path: Option<String>,
    pub srt_path: Option<String>,
    pub text_path: Option<String>,
}

/// 下载 YouTube 字幕
#[tauri::command]
pub async fn download_youtube_subtitles(
    url: String,
    output_dir: String,
    python_path: String,
    languages: Option<String>,
) -> Result<SubtitleResult, AppError> {
    let mut args = vec![url, "--output-dir".into(), output_dir];
    if let Some(langs) = languages {
        args.push("--languages".into());
        args.push(langs);
    }

    run_and_parse(
        &python_path,
        "download_youtube_subtitles.py",
        &args,
        "YouTube 字幕",
    )
    .await
}

// ---- 5. install_faster_whisper.py ----

/// faster-whisper 安装结果
#[derive(Debug, Serialize, Deserialize)]
pub struct InstallWhisperResult {
    pub venv_python: String,
    pub mirror: String,
    pub index_url: String,
    pub versions: serde_json::Value,
}

/// 安装 faster-whisper 到 venv
#[tauri::command]
pub async fn install_faster_whisper(
    python_path: String,
    venv_dir: Option<String>,
) -> Result<InstallWhisperResult, AppError> {
    let mut args = Vec::new();
    if let Some(dir) = venv_dir {
        args.push("--venv-dir".into());
        args.push(dir);
    }

    run_and_parse(
        &python_path,
        "install_faster_whisper.py",
        &args,
        "faster-whisper 安装",
    )
    .await
}

// ---- 6. list_ai_douyin_tasks.py ----

/// AI Douyin 任务列表结果
#[derive(Debug, Serialize, Deserialize)]
pub struct AiDouyinTaskList {
    // API 返回的 JSON 结构，此处用 Value 兜底
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// 查询 AI Douyin 任务列表
#[tauri::command]
pub async fn list_ai_douyin_tasks(
    python_path: String,
    api_key: String,
    api_base: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<String>,
    search: Option<String>,
) -> Result<AiDouyinTaskList, AppError> {
    let mut args = vec!["--api-key".into(), api_key, "--json".into()];

    if let Some(base) = api_base {
        args.push("--api-base".into());
        args.push(base);
    }
    if let Some(p) = page {
        args.push("--page".into());
        args.push(p.to_string());
    }
    if let Some(ps) = page_size {
        args.push("--page-size".into());
        args.push(ps.to_string());
    }
    if let Some(s) = status {
        args.push("--status".into());
        args.push(s);
    }
    if let Some(q) = search {
        args.push("--search".into());
        args.push(q);
    }

    run_and_parse(
        &python_path,
        "list_ai_douyin_tasks.py",
        &args,
        "AI Douyin 任务",
    )
    .await
}

// ============================================================
// Python 环境检测
// ============================================================

/// 检测 Python 环境
#[tauri::command]
pub async fn check_python_env(python_path: String) -> Result<String, AppError> {
    let (program, prefix_args) = python_command_parts(&python_path);
    let output = Command::new(program)
        .args(prefix_args)
        .args(["-c", "import sys; print(sys.version)"])
        .output()
        .await
        .map_err(|_| {
            AppError::MissingDependency(format!("无法运行指定的 Python: {python_path}"))
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(AppError::MissingDependency(format!(
            "无法运行指定的 Python: {python_path}"
        )))
    }
}

/// 获取 faster-whisper venv 的 Python 路径
///
/// 默认 venv 目录: `~/.cache/myriad-mind/faster-whisper-venv/`
/// 返回 venv 中 python 可执行文件的路径，venv 不存在时返回 None
pub fn get_whisper_venv_python() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    let venv_dir = PathBuf::from(home)
        .join(".cache")
        .join("myriad-mind")
        .join("faster-whisper-venv");

    let python_bin = if cfg!(target_os = "windows") {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    };

    if python_bin.exists() {
        Some(python_bin)
    } else {
        None
    }
}

/// 解析用户配置的 Python 路径或自动探测
///
/// 优先级:
/// 1. 用户配置的 python_path
/// 2. FW_PYTHON 环境变量
/// 3. faster-whisper venv 的 Python
/// 4. Windows 常见 Python 安装目录
/// 5. py -3 / python / python3
pub fn resolve_python_path(configured: Option<&str>) -> String {
    // 1. 用户显式配置
    if let Some(path) = configured {
        let path = path.trim();
        if !path.is_empty() && python_path_works(path) {
            return path.to_string();
        }
    }

    // 2. 环境变量指定
    if let Ok(path) = std::env::var("FW_PYTHON") {
        let path = path.trim();
        if !path.is_empty() && python_path_works(path) {
            return path.to_string();
        }
    }

    // 3. faster-whisper venv
    if let Some(venv_python) = get_whisper_venv_python() {
        let path = venv_python.to_string_lossy().to_string();
        if python_path_works(&path) {
            return path;
        }
    }

    // 4. Windows 常见安装路径。优先于 PATH，避免 python/python3 命中 Store stub。
    #[cfg(target_os = "windows")]
    {
        for path in common_windows_python_paths() {
            let path = path.to_string_lossy().to_string();
            if python_path_works(&path) {
                return path;
            }
        }

        if python_works("py", &["-3"]) {
            return "py -3".to_string();
        }

        if python_works("python", &[]) {
            return "python".to_string();
        }

        return "python".to_string();
    }

    // 5. 非 Windows PATH
    #[cfg(not(target_os = "windows"))]
    {
        if python_works("python3", &[]) {
            return "python3".to_string();
        }
        if python_works("python", &[]) {
            return "python".to_string();
        }

        "python3".to_string()
    }
}

/// Split a Python command returned by resolve_python_path into program + prefix args.
/// This keeps support for the Windows launcher form `py -3`.
pub fn python_command_parts(python_path: &str) -> (String, Vec<String>) {
    if python_path == "py -3" {
        ("py".into(), vec!["-3".into()])
    } else {
        (python_path.to_string(), vec![])
    }
}

fn python_works(program: &str, prefix_args: &[&str]) -> bool {
    let mut cmd = StdCommand::new(program);
    cmd.args(prefix_args).arg("--version");
    match cmd.output() {
        Ok(output) if output.status.success() => {
            let combined = format!(
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            python_version_at_least(&combined, PYTHON_MIN_VERSION)
        }
        _ => false,
    }
}

fn python_version_at_least(output: &str, min: (u32, u32)) -> bool {
    parse_python_version_output(output)
        .map(|(major, minor, _)| (major, minor) >= min)
        .unwrap_or(false)
}

fn parse_python_version_output(output: &str) -> Option<(u32, u32, u32)> {
    for token in output.split_whitespace() {
        if let Some(version) = parse_python_version_token(token) {
            return Some(version);
        }
    }
    None
}

fn parse_python_version_token(token: &str) -> Option<(u32, u32, u32)> {
    let token = token.trim_start_matches("Python").trim();
    let mut parts = token.split('.');
    let major = parse_numeric_part(parts.next()?)?;
    let minor = parse_numeric_part(parts.next()?)?;
    let patch = parts.next().and_then(parse_numeric_part).unwrap_or(0);
    Some((major, minor, patch))
}

fn parse_numeric_part(part: &str) -> Option<u32> {
    let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn python_path_works(python_path: &str) -> bool {
    let (program, args) = python_command_parts(python_path);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    python_works(&program, &arg_refs)
}

#[cfg(target_os = "windows")]
fn common_windows_python_paths() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let programs = PathBuf::from(local_app_data)
            .join("Programs")
            .join("Python");
        collect_python_exes(&programs, &mut candidates);
    }

    if let Ok(program_files) = std::env::var("ProgramFiles") {
        collect_python_exes(
            &PathBuf::from(program_files).join("Python"),
            &mut candidates,
        );
    }

    if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
        collect_python_exes(
            &PathBuf::from(program_files_x86).join("Python"),
            &mut candidates,
        );
    }

    candidates.sort_by(|a, b| b.cmp(a));
    candidates
}

#[cfg(target_os = "windows")]
fn collect_python_exes(base: &PathBuf, candidates: &mut Vec<PathBuf>) {
    if !base.exists() {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let exe = path.join("python.exe");
                if exe.exists() {
                    candidates.push(exe);
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn common_windows_python_paths() -> Vec<PathBuf> {
    Vec::new()
}
