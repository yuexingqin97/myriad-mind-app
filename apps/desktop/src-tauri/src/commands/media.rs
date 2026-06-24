// ============================================================
// 关键帧抽取 — Rust 直调 FFmpeg（原 extract_keyframes.py）
//
// 取代 Python 中转：用 std::process::Command 直接调度 FFmpeg。
// 全部逻辑（interval / scene / both / guided、两遍 scene 检测、
// pts_time 解析、去重排序、写 keyframes.json）与原脚本逐字对齐
// （见《Python到Rust迁移计划》§6.4）。
//
// 与 pipeline.rs::extract_audio_ffmpeg 同属「Rust 直调 FFmpeg」操作。
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;

// ============================================================
// IPC 返回类型（自 python.rs 迁入；与原 JSON 形状一致）
// ============================================================

/// 关键帧提取结果（Tauri 命令返回；keyframes 不含 trigger，与原 serde 行为一致）
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

// ============================================================
// 内部：带 trigger 的关键帧（落盘 keyframes.json 用，vision.rs 依赖 trigger）
// ============================================================

#[derive(Debug, Clone)]
pub(crate) struct KeyframeOut {
    file: String,
    timestamp_seconds: f64,
    timestamp_label: String,
    trigger: String,
}

impl From<KeyframeOut> for KeyframeInfo {
    fn from(k: KeyframeOut) -> Self {
        // IPC 返回不含 trigger（与原 python.rs KeyframeInfo 行为一致）
        KeyframeInfo {
            file: k.file,
            timestamp_seconds: k.timestamp_seconds,
            timestamp_label: k.timestamp_label,
        }
    }
}

// ============================================================
// FFmpeg 可执行文件解析（与 pipeline.rs 同款：PATH + WinGet 兜底）
// ============================================================

fn apply_windows_no_window(cmd: &mut std::process::Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

fn command_works(cmd: &str, args: &[&str]) -> bool {
    let mut c = std::process::Command::new(cmd);
    apply_windows_no_window(&mut c);
    c.args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn resolve_ffmpeg_binary(name: &str) -> Option<String> {
    let candidates = if cfg!(target_os = "windows") {
        vec![format!("{name}.exe"), name.to_string()]
    } else {
        vec![name.to_string()]
    };

    for candidate in candidates {
        if command_works(&candidate, &["-version"]) {
            return Some(candidate);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_base = std::path::PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if let Ok(entries) = std::fs::read_dir(winget_base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let pkg_name = entry.file_name().to_string_lossy().to_lowercase();
                    if !pkg_name.contains("ffmpeg") {
                        continue;
                    }
                    for nested in [
                        path.join("bin").join(format!("{name}.exe")),
                        path.join(format!("{name}.exe")),
                    ] {
                        if nested.exists() {
                            return Some(nested.to_string_lossy().to_string());
                        }
                    }
                    if let Ok(walk) = std::fs::read_dir(&path) {
                        for child in walk.flatten() {
                            let exe = child.path().join(format!("{name}.exe"));
                            if exe.exists() {
                                return Some(exe.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

// ============================================================
// 纯工具函数（忠实移植脚本同名函数）
// ============================================================

/// timestamp_label：HHhMMmSSs（≥1h）/ MMmSSs（脚本 timestamp_label）
fn timestamp_label(seconds: f64) -> String {
    let s = (seconds.round() as i64).max(0);
    let hours = s / 3600;
    let minutes = (s % 3600) / 60;
    let secs = s % 60;
    if hours > 0 {
        format!("{hours:02}h{minutes:02}m{secs:02}s")
    } else {
        format!("{minutes:02}m{secs:02}s")
    }
}

/// slug_reason：空白→_，去非 [\w CJK _ -]，截 24 字符，空 → "guided"
/// （脚本 slug_reason；不引入 regex crate，手写字符过滤）
fn slug_reason(reason: &str) -> String {
    let trimmed = reason.trim();
    let mut out = String::with_capacity(trimmed.len());
    let mut prev_under = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !prev_under {
                out.push('_');
                prev_under = true;
            }
            continue;
        }
        prev_under = false;
        // \w ∪ CJK ∪ '_' ∪ '-'；\w 在 unicode 下 ≈ is_alphanumeric + '_'
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        }
        // 其余字符丢弃（对应 re.sub 的删除）
    }
    let truncated: String = out.chars().take(24).collect();
    if truncated.is_empty() {
        "guided".to_string()
    } else {
        truncated
    }
}

/// 从 showinfo stderr 解析 pts_time（脚本 extract_scene_frames Pass1 解析）
fn parse_pts_times(stderr: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for line in stderr.lines() {
        if let Some(idx) = line.find("pts_time:") {
            let rest = &line[idx + "pts_time:".len()..];
            let num: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(f) = num.parse::<f64>() {
                out.push(f);
            }
        }
    }
    out
}

/// load_guided_timestamps：读 JSON（array 或 {timestamps:[...]}），
/// 解析 (ts, reason)，按 ts 排序、去 <0、去 2s 内重复（脚本同名函数）
fn load_guided_timestamps(path: Option<&str>) -> Result<Vec<(f64, String)>, AppError> {
    let path = match path {
        Some(p) if !p.is_empty() => p,
        _ => return Ok(Vec::new()),
    };
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()), // 文件缺失 → 空（与脚本一致）
    };
    let parsed: serde_json::Value = serde_json::from_str(&raw)?;

    // items = raw.timestamps（dict 时）否则 raw（array 时）
    let items = match &parsed {
        serde_json::Value::Object(map) => map.get("timestamps").cloned().unwrap_or(parsed.clone()),
        other => other.clone(),
    };

    let mut result: Vec<(f64, String)> = Vec::new();
    if let Some(arr) = items.as_array() {
        for item in arr {
            match item {
                serde_json::Value::Number(_) => {
                    if let Some(f) = to_f64(item) {
                        result.push((f, "AI推荐".to_string()));
                    }
                }
                serde_json::Value::Object(obj) => {
                    // ts = obj.ts ?? obj.timestamp ?? obj.timestamp_seconds
                    let ts_v = obj
                        .get("ts")
                        .or_else(|| obj.get("timestamp"))
                        .or_else(|| obj.get("timestamp_seconds"));
                    if let Some(ts) = ts_v.and_then(to_f64) {
                        let reason = obj
                            .get("reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("AI推荐")
                            .to_string();
                        result.push((ts, reason));
                    }
                }
                _ => {}
            }
        }
    }

    // 按 ts 排序；去 <0；去 2s 内重复
    result.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut deduped: Vec<(f64, String)> = Vec::new();
    for (ts, reason) in result {
        if ts < 0.0 {
            continue;
        }
        if let Some(last) = deduped.last() {
            if (last.0 - ts).abs() < 2.0 {
                continue;
            }
        }
        deduped.push((ts, reason));
    }
    Ok(deduped)
}

/// 兼容 number / 数字字符串 → f64（Python float(item) 对 str 也尝试转换）
fn to_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
}

// ============================================================
// FFmpeg 截帧子操作（argv 与脚本逐字一致）
// ============================================================

/// 单帧精准截取：-ss {ts:.3} -i {video} -frames:v 1 -q:v 2 -y {out}
/// （guided / scene Pass2 共用；check=True 语义：非零退出即失败）
fn run_ffmpeg_single(ffmpeg: &str, ts: f64, video: &Path, output: &Path) -> Result<(), AppError> {
    let mut cmd = std::process::Command::new(ffmpeg);
    apply_windows_no_window(&mut cmd);
    cmd.arg("-ss")
        .arg(format!("{:.3}", ts))
        .arg("-i")
        .arg(video)
        .arg("-frames:v")
        .arg("1")
        .arg("-q:v")
        .arg("2")
        .arg("-y")
        .arg(output)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let out = cmd.output().map_err(AppError::Io)?;
    if !out.status.success() {
        let se = String::from_utf8_lossy(&out.stderr);
        let se_short: String = se.chars().take(200).collect();
        return Err(AppError::Other(format!(
            "ffmpeg 截帧失败 (ts={ts:.3}): {se_short}"
        )));
    }
    Ok(())
}

/// guided：按时间点逐帧截取，trigger=guided:{reason}（脚本 extract_guided_frames）
fn extract_guided_frames(
    video: &Path,
    frames_dir: &Path,
    timestamps: &[(f64, String)],
    max_frames: usize,
    ffmpeg: &str,
) -> Result<Vec<KeyframeOut>, AppError> {
    std::fs::create_dir_all(frames_dir)?;
    let mut out = Vec::new();
    let take = timestamps.len().min(max_frames);
    for (index, (ts, reason)) in timestamps[..take].iter().enumerate() {
        let i = index + 1; // 1-based，%04d
        let fname = format!("guided_{i:04}_{}.png", slug_reason(reason));
        let opath = frames_dir.join(&fname);
        run_ffmpeg_single(ffmpeg, *ts, video, &opath)?;
        if opath.exists() {
            out.push(KeyframeOut {
                file: fname,
                timestamp_seconds: *ts,
                timestamp_label: timestamp_label(*ts),
                trigger: format!("guided:{reason}"),
            });
        }
    }
    Ok(out)
}

/// interval：fps 滤镜固定间隔截帧，trigger=interval（脚本 extract_interval_frames）
fn extract_interval_frames(
    video: &Path,
    frames_dir: &Path,
    interval: u32,
    max_frames: u32,
    ffmpeg: &str,
) -> Result<Vec<KeyframeOut>, AppError> {
    std::fs::create_dir_all(frames_dir)?;
    let fps = 1.0 / interval as f64;
    let filter_v = format!("fps={fps:.6}");
    let pattern = frames_dir.join("frame_%04d.png");

    let mut cmd = std::process::Command::new(ffmpeg);
    apply_windows_no_window(&mut cmd);
    cmd.arg("-i")
        .arg(video)
        .arg("-vf")
        .arg(&filter_v)
        .arg("-frames:v")
        .arg(max_frames.to_string())
        .arg("-q:v")
        .arg("2")
        .arg("-y")
        .arg(&pattern)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let out = cmd.output().map_err(AppError::Io)?;
    if !out.status.success() {
        let se = String::from_utf8_lossy(&out.stderr);
        let se_short: String = se.chars().take(200).collect();
        return Err(AppError::Other(format!(
            "ffmpeg interval 截帧失败: {se_short}"
        )));
    }

    // 收集 frame_*.png，按文件名排序，解析序号回算时间戳
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(frames_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            n.starts_with("frame_") && n.ends_with(".png")
        })
        .collect();
    files.sort();

    let mut keyframes = Vec::new();
    for png in files {
        let name = png
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let stem = png
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(); // "frame_0001"
        let num: i64 = stem
            .split('_')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let ts = (num - 1) * interval as i64;
        keyframes.push(KeyframeOut {
            file: name,
            timestamp_seconds: ts as f64,
            timestamp_label: timestamp_label(ts as f64),
            trigger: "interval".into(),
        });
    }
    Ok(keyframes)
}

/// scene：两遍——Pass1 select+showinfo 探测 pts_time（exit 忽略），
/// Pass2 按时间点精准截单帧，trigger=scene（脚本 extract_scene_frames）
fn extract_scene_frames(
    video: &Path,
    frames_dir: &Path,
    max_frames: u32,
    ffmpeg: &str,
    threshold: f64,
    min_gap: f64,
    _max_gap: f64, // 脚本签名保留 max_gap，实际未在截帧逻辑使用（与脚本一致）
) -> Result<Vec<KeyframeOut>, AppError> {
    std::fs::create_dir_all(frames_dir)?;

    // ---- Pass 1：探测场景变化 pts_time（输出到 null，解析 stderr）----
    let null_dev = if cfg!(target_os = "windows") { "NUL" } else { "/dev/null" };
    // 单 argv 元素，反斜杠转义逗号（与脚本 f"select=gt(scene\\,{threshold}),showinfo" 一致）
    let filter_v = format!("select=gt(scene\\,{threshold}),showinfo");
    let mut cmd = std::process::Command::new(ffmpeg);
    apply_windows_no_window(&mut cmd);
    cmd.arg("-i")
        .arg(video)
        .arg("-vf")
        .arg(&filter_v)
        .arg("-vsync")
        .arg("vfr")
        .arg("-f")
        .arg("null")
        .arg("-y")
        .arg(null_dev)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let result = cmd.output().map_err(AppError::Io)?;
    // 注意：Pass1 不检查 exit code（与脚本一致，showinfo 探测可能非 0 退出但 stderr 仍可用）
    let stderr = String::from_utf8_lossy(&result.stderr);
    let pts_times = parse_pts_times(&stderr);
    if pts_times.is_empty() {
        return Ok(Vec::new()); // 未检测到场景变化
    }

    // 去重：连续时间戳间隔 < min_gap 跳过（last_ts 初值 -min_gap-1 保证首个通过）
    let mut deduped: Vec<f64> = Vec::new();
    let mut last_ts = -min_gap - 1.0;
    for pts in pts_times {
        if pts - last_ts < min_gap {
            continue;
        }
        deduped.push(pts);
        last_ts = pts;
    }
    let maxf = max_frames as usize;
    let timestamps: Vec<f64> = deduped.into_iter().take(maxf).collect();

    // ---- Pass 2：按时间点精准截单帧 ----
    let mut keyframes = Vec::new();
    for (index, ts) in timestamps.iter().enumerate() {
        let i = index + 1;
        let safe_tag = format!("{ts:.1}s").replace('.', "_");
        let fname = format!("scene_{i:04}_{safe_tag}.png");
        let opath = frames_dir.join(&fname);
        run_ffmpeg_single(ffmpeg, *ts, video, &opath)?;
        if opath.exists() {
            keyframes.push(KeyframeOut {
                file: fname,
                timestamp_seconds: *ts,
                timestamp_label: timestamp_label(*ts),
                trigger: "scene".into(),
            });
        }
    }
    Ok(keyframes)
}

// ============================================================
// 核心：extract_keyframes（脚本 extract_keyframes）
// ============================================================

/// 关键帧 index JSON 构造（含 trigger，落盘 keyframes.json 用）
fn keyframes_index_value(unique: &[KeyframeOut]) -> serde_json::Value {
    serde_json::Value::Array(
        unique
            .iter()
            .map(|k| {
                serde_json::json!({
                    "file": k.file,
                    "timestamp_seconds": k.timestamp_seconds,
                    "timestamp_label": k.timestamp_label,
                    "trigger": k.trigger,
                })
            })
            .collect(),
    )
}

/// 关键帧抽取核心实现（Rust 直调 FFmpeg，无 Python 中转）。
///
/// 落盘 `{output_dir}/frames/keyframes.json`（含 trigger），返回去重排序后的帧清单。
/// 参数语义与脚本 extract_keyframes 逐字对齐。
pub(crate) fn extract_keyframes_impl(
    video_path: &str,
    output_dir: &Path,
    mode: &str,
    interval: u32,
    max_frames: u32,
    timestamps_path: Option<&str>,
    scene_threshold: f64,
    min_gap: f64,
    max_gap: f64,
) -> Result<Vec<KeyframeOut>, AppError> {
    let ffmpeg = resolve_ffmpeg_binary("ffmpeg")
        .ok_or_else(|| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;

    let video = Path::new(video_path);
    if !video.exists() {
        return Err(AppError::Other(format!(
            "Video file not found: {video_path}"
        )));
    }

    let frames_dir = output_dir.join("frames");
    std::fs::create_dir_all(&frames_dir)?;
    let mut all: Vec<KeyframeOut> = Vec::new();

    // guided（字幕引导时间点）
    let guided = load_guided_timestamps(timestamps_path)?;
    if !guided.is_empty() {
        let g = extract_guided_frames(video, &frames_dir, &guided, max_frames as usize, &ffmpeg)?;
        all.extend(g);
    }

    // interval / both
    if mode == "interval" || mode == "both" {
        let iv = extract_interval_frames(video, &frames_dir, interval, max_frames, &ffmpeg)?;
        all.extend(iv);
    }

    // scene / both（both 模式 scene 配额 = max(max_frames//3, 5)）
    if mode == "scene" || mode == "both" {
        let scene_max = if mode == "scene" {
            max_frames
        } else {
            std::cmp::max(max_frames / 3, 5)
        };
        let sf = extract_scene_frames(
            video,
            &frames_dir,
            scene_max,
            &ffmpeg,
            scene_threshold,
            min_gap,
            max_gap,
        )?;
        all.extend(sf);
    }

    // 去重（按文件名）+ 按时间戳排序
    all.sort_by(|a, b| {
        a.timestamp_seconds
            .partial_cmp(&b.timestamp_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<KeyframeOut> = Vec::new();
    for kf in all {
        if seen.insert(kf.file.clone()) {
            unique.push(kf);
        }
    }
    let maxf = max_frames as usize;
    unique.truncate(maxf);

    // 写 keyframes.json（含 trigger；ensure_ascii=False + indent=2 等价）
    std::fs::create_dir_all(&frames_dir)?;
    let index_path = frames_dir.join("keyframes.json");
    let pretty = serde_json::to_string_pretty(&keyframes_index_value(&unique))?;
    std::fs::write(&index_path, pretty)?;

    Ok(unique)
}

/// 环境变量 → f64，缺失/空/非法回退默认（移植脚本 env_or_default 用于 scene 阈值）
fn env_f64(name: &str, default: f64) -> f64 {
    match std::env::var(name) {
        Ok(v) => {
            let cleaned = v.trim();
            if cleaned.is_empty() {
                return default;
            }
            cleaned.parse::<f64>().unwrap_or(default)
        }
        Err(_) => default,
    }
}

/// 执行关键帧提取（Tauri 命令；Rust 直调 FFmpeg，无 Python 中转）
///
/// 同步命令：FFmpeg 为阻塞子进程，Tauri 将同步命令调度到阻塞线程池，
/// 避免占用异步运行时（原 async 仅因 Python 中转而生）。
#[tauri::command]
pub fn extract_keyframes(
    video_path: String,
    output_dir: String,
    interval: u32,
    max_frames: u32,
    mode: String,
) -> Result<KeyframeResult, AppError> {
    // scene 阈值族：脚本 argparse 默认（可被 KF_* 环境变量覆盖，移植 env_or_default）
    let scene_threshold = env_f64("KF_SCENE_THRESHOLD", 0.25);
    let min_gap = env_f64("KF_MIN_GAP", 3.0);
    let max_gap = env_f64("KF_MAX_GAP", 120.0);

    log::debug!(
        target: "agent",
        "[media] phase=extract_keyframes video={video_path} mode={mode} interval={interval} max_frames={max_frames}"
    );
    let proc_start = std::time::Instant::now();

    let frames = extract_keyframes_impl(
        &video_path,
        Path::new(&output_dir),
        &mode,
        interval,
        max_frames,
        None, // Tauri 命令路径不传 guided 时间戳（与原命令一致）
        scene_threshold,
        min_gap,
        max_gap,
    )?;

    log::debug!(
        target: "agent",
        "[media] phase=done frames={} duration_ms={}",
        frames.len(),
        proc_start.elapsed().as_millis()
    );

    Ok(KeyframeResult {
        result: KeyframeData {
            video_path,
            output_dir,
            mode,
            interval,
            max_frames,
            keyframes: frames.into_iter().map(KeyframeInfo::from).collect(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_label_minutes() {
        assert_eq!(timestamp_label(183.0), "03m03s");
        assert_eq!(timestamp_label(0.0), "00m00s");
        assert_eq!(timestamp_label(59.4), "00m59s");
        assert_eq!(timestamp_label(60.0), "01m00s");
    }

    #[test]
    fn timestamp_label_hours() {
        assert_eq!(timestamp_label(3725.0), "01h02m05s");
    }

    #[test]
    fn timestamp_label_negative_clamps() {
        assert_eq!(timestamp_label(-5.0), "00m00s");
    }

    #[test]
    fn slug_reason_basic() {
        assert_eq!(slug_reason("AI 推荐!"), "AI_推荐");
        assert_eq!(slug_reason(""), "guided");
        assert_eq!(slug_reason("   "), "guided");
    }

    #[test]
    fn slug_reason_truncates_24() {
        let long = "a".repeat(40);
        assert_eq!(slug_reason(&long).chars().count(), 24);
    }

    #[test]
    fn parse_pts_times_extracts_numbers() {
        let stderr = "frame=123 pts_time:183.456\njunk\npts_time:foo\nx pts_time:12.0 y";
        let pts = parse_pts_times(stderr);
        assert_eq!(pts, vec![183.456, 12.0]);
    }

    #[test]
    fn parse_pts_times_empty() {
        assert!(parse_pts_times("no showinfo here").is_empty());
    }

    #[test]
    fn keyframes_index_includes_trigger() {
        // vision.rs 依赖 keyframes.json 含 trigger 字段——断言落盘结构正确
        let frames = vec![
            KeyframeOut {
                file: "scene_0001_183_5s.png".into(),
                timestamp_seconds: 183.5,
                timestamp_label: "03m04s".into(),
                trigger: "scene".into(),
            },
            KeyframeOut {
                file: "guided_0001_AI.png".into(),
                timestamp_seconds: 10.0,
                timestamp_label: "00m10s".into(),
                trigger: "guided:AI推荐".into(),
            },
        ];
        let v = keyframes_index_value(&frames);
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["trigger"].as_str(), Some("scene"));
        assert_eq!(arr[1]["trigger"].as_str(), Some("guided:AI推荐"));
        assert_eq!(arr[0]["file"].as_str(), Some("scene_0001_183_5s.png"));
    }

    #[test]
    fn to_f64_handles_string_and_number() {
        assert_eq!(to_f64(&serde_json::json!(12.5)), Some(12.5));
        assert_eq!(to_f64(&serde_json::json!(12)), Some(12.0));
        assert_eq!(to_f64(&serde_json::json!("42")), Some(42.0));
        assert_eq!(to_f64(&serde_json::json!("notnum")), None);
    }

    #[test]
    fn load_guided_timestamps_parses_objects_and_numbers() {
        let dir = std::env::temp_dir().join(format!("mm_kf_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ts.json");
        std::fs::write(
            &path,
            r#"{"timestamps":[{"ts":183.5,"reason":"场景"},{"ts":5},{"ts":-1},{"ts":184.0}]}"#,
        )
        .unwrap();
        let res = load_guided_timestamps(path.to_str()).unwrap();
        // 排序后 5, 183.5, 184.0；184.0 距 183.5 < 2 去重；-1 去除
        assert_eq!(res.len(), 2);
        assert!((res[0].0 - 5.0).abs() < 1e-9);
        assert!((res[1].0 - 183.5).abs() < 1e-9);
        assert_eq!(res[1].1, "场景");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
