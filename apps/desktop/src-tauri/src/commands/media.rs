// ============================================================
// 关键帧提取 — Rust 直调 FFmpeg，替代 extract_keyframes.py
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---- 数据结构（与 Python 脚本输出一致）----

/// 单张关键帧
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub file: String,
    pub timestamp_seconds: f64,
    pub timestamp_label: String,
    pub trigger: String,
}

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
    pub keyframes: Vec<Keyframe>,
}

/// 引导时间点
#[derive(Debug, Clone)]
pub struct GuidedTimestamp {
    pub ts: f64,
    pub reason: String,
}

// ============================================================
// FFmpeg 定位（与 pipeline.rs / deps.rs 的 resolve_ffmpeg_binary 一致）
// ============================================================

/// 定位 ffmpeg 或 ffprobe 可执行文件
pub fn resolve_ffmpeg_binary(name: &str) -> Option<String> {
    let candidates = if cfg!(target_os = "windows") {
        vec![format!("{name}.exe"), name.to_string()]
    } else {
        vec![name.to_string()]
    };

    for candidate in candidates {
        if let Ok(output) = Command::new(&candidate).arg("-version").output() {
            if output.status.success() {
                return Some(candidate);
            }
        }
    }

    // 检查 Winget 安装路径（Windows）
    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_base = PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if winget_base.exists() {
                if let Ok(entries) = std::fs::read_dir(&winget_base) {
                    for entry in entries.flatten() {
                        let pkg = entry.path();
                        let pkg_name = pkg
                            .file_name()
                            .map(|n| n.to_string_lossy().to_lowercase())
                            .unwrap_or_default();
                        if pkg_name.contains("ffmpeg") {
                            // 直接在包目录下找
                            for exe_name in &[format!("{name}.exe"), name.to_string()] {
                                let candidate = pkg.join(&exe_name);
                                if candidate.exists() {
                                    return Some(candidate.to_string_lossy().to_string());
                                }
                                // 也尝试 bin/ 子目录
                                let bin_candidate = pkg.join("bin").join(&exe_name);
                                if bin_candidate.exists() {
                                    return Some(bin_candidate.to_string_lossy().to_string());
                                }
                            }
                            // 递归搜索
                            if let Some(found) = find_exe_recursive_impl(&pkg, name) {
                                return Some(found);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn find_exe_recursive(dir: &Path, name: &str) -> Option<String> {
    find_exe_recursive_impl(dir, name)
}

#[cfg(target_os = "windows")]
fn find_exe_recursive_impl(dir: &Path, name: &str) -> Option<String> {
    use std::fs;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_exe_recursive(&path, name) {
                    return Some(found);
                }
            } else if let Some(fname) = path.file_name() {
                let fname_lower = fname.to_string_lossy().to_lowercase();
                if fname_lower == format!("{name}.exe").to_lowercase() {
                    return Some(path.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

/// 获取视频时长（ffprobe）
pub fn get_video_duration(video_path: &Path, ffprobe_bin: &str) -> Option<f64> {
    let output = Command::new(ffprobe_bin)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            &video_path.to_string_lossy(),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let info: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    info.get("format")?
        .get("duration")?
        .as_str()?
        .parse::<f64>()
        .ok()
}

// ============================================================
// 辅助函数（与 Python 脚本逻辑一致）
// ============================================================

/// 时间戳 → 标签（如 "03m03s" 或 "01h02m03s"）
fn timestamp_label(seconds: f64) -> String {
    let total_secs = (seconds.round() as i64).max(0);
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{hours:02}h{minutes:02}m{secs:02}s")
    } else {
        format!("{minutes:02}m{secs:02}s")
    }
}

/// 原因 → slug（文件名安全，最多 24 字符）
fn slug_reason(reason: &str) -> String {
    let cleaned: String = reason
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                '_'
            } else if c.is_alphanumeric() || c == '_' || c == '-' || (c as u32) >= 0x4e00
            {
                c
            } else {
                '_'
            }
        })
        .collect();
    let slug = cleaned.trim_matches('_');
    if slug.len() <= 24 {
        slug.to_string()
    } else {
        slug[..24].to_string()
    }
}

/// 加载引导时间点 JSON 文件
pub fn load_guided_timestamps(path: Option<&Path>) -> Vec<GuidedTimestamp> {
    let ts_path = match path {
        Some(p) if p.exists() => p,
        _ => return vec![],
    };

    let raw = match std::fs::read_to_string(ts_path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let items = match &json {
        serde_json::Value::Object(map) => map.get("timestamps").unwrap_or(&json),
        _ => &json,
    };

    let array = match items {
        serde_json::Value::Array(arr) => arr,
        _ => return vec![],
    };

    let mut result: Vec<GuidedTimestamp> = Vec::new();
    for item in array {
        match item {
            serde_json::Value::Number(n) => {
                if let Some(ts) = n.as_f64() {
                    result.push(GuidedTimestamp {
                        ts,
                        reason: "AI推荐".to_string(),
                    });
                }
            }
            serde_json::Value::Object(obj) => {
                let ts = obj
                    .get("ts")
                    .or_else(|| obj.get("timestamp"))
                    .or_else(|| obj.get("timestamp_seconds"))
                    .and_then(|v| v.as_f64());
                if let Some(ts) = ts {
                    let reason = obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("AI推荐")
                        .to_string();
                    result.push(GuidedTimestamp { ts, reason });
                }
            }
            _ => {}
        }
    }

    // 排序 + 去重（2 秒内视为重复）+ 过滤负数
    result.sort_by(|a, b| a.ts.partial_cmp(&b.ts).unwrap_or(std::cmp::Ordering::Equal));
    let mut deduped: Vec<GuidedTimestamp> = Vec::new();
    for gts in result {
        if gts.ts < 0.0 {
            continue;
        }
        if let Some(last) = deduped.last() {
            if (gts.ts - last.ts).abs() < 2.0 {
                continue;
            }
        }
        deduped.push(gts);
    }
    deduped
}

// ============================================================
// 核心提取函数（与 Python 脚本逻辑一致）
// ============================================================

/// 引导时间点截图
fn extract_guided_frames(
    video_path: &Path,
    output_dir: &Path,
    timestamps: &[GuidedTimestamp],
    max_frames: usize,
    ffmpeg_bin: &str,
) -> Result<Vec<Keyframe>, AppError> {
    std::fs::create_dir_all(output_dir).map_err(AppError::Io)?;
    let mut keyframes = Vec::new();

    for (index, gts) in timestamps.iter().take(max_frames).enumerate() {
        let idx = index + 1;
        let slug = slug_reason(&gts.reason);
        let output = output_dir.join(format!("guided_{idx:04}_{slug}.png"));

        let status = apply_no_window(
            Command::new(ffmpeg_bin)
                .args([
                    "-ss",
                    &format!("{:.3}", gts.ts),
                    "-i",
                    &video_path.to_string_lossy(),
                    "-frames:v",
                    "1",
                    "-q:v",
                    "2",
                    "-y",
                    &output.to_string_lossy(),
                ]),
        )
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| AppError::MissingDependency("FFmpeg 执行失败".into()))?;

        if status.success() && output.exists() {
            keyframes.push(Keyframe {
                file: output
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                timestamp_seconds: gts.ts,
                timestamp_label: timestamp_label(gts.ts),
                trigger: format!("guided:{}", gts.reason),
            });
        }
    }

    Ok(keyframes)
}

/// 固定间隔截图
fn extract_interval_frames(
    video_path: &Path,
    output_dir: &Path,
    interval: u32,
    max_frames: u32,
    ffmpeg_bin: &str,
) -> Result<Vec<Keyframe>, AppError> {
    std::fs::create_dir_all(output_dir).map_err(AppError::Io)?;

    let fps = 1.0 / interval as f64;
    let filter_v = format!("fps={fps:.6}");
    let pattern = output_dir.join("frame_%04d.png");

    let status = apply_no_window(
        Command::new(ffmpeg_bin)
            .args([
                "-i",
                &video_path.to_string_lossy(),
                "-vf",
                &filter_v,
                "-frames:v",
                &max_frames.to_string(),
                "-q:v",
                "2",
                "-y",
                &pattern.to_string_lossy(),
            ]),
    )
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status()
    .map_err(|_| AppError::MissingDependency("FFmpeg 执行失败".into()))?;

    if !status.success() {
        return Err(AppError::Other("FFmpeg 间隔截图失败".into()));
    }

    // 收集生成的帧
    let mut keyframes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(output_dir) {
        let mut pngs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().starts_with("frame_"))
                    .unwrap_or(false)
            })
            .collect();
        pngs.sort();

        for png in pngs {
            let stem = png.file_stem().unwrap_or_default().to_string_lossy();
            if let Some(num_str) = stem.split('_').nth(1) {
                if let Ok(num) = num_str.parse::<u32>() {
                    let ts = (num.saturating_sub(1)) as f64 * interval as f64;
                    keyframes.push(Keyframe {
                        file: png
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        timestamp_seconds: ts,
                        timestamp_label: timestamp_label(ts),
                        trigger: "interval".to_string(),
                    });
                }
            }
        }
    }

    Ok(keyframes)
}

/// 场景变化检测截图（两遍法）
fn extract_scene_frames(
    video_path: &Path,
    output_dir: &Path,
    max_frames: usize,
    ffmpeg_bin: &str,
    threshold: f64,
    min_gap: f64,
    _max_gap: f64,
) -> Result<Vec<Keyframe>, AppError> {
    std::fs::create_dir_all(output_dir).map_err(AppError::Io)?;

    // Pass 1: 检测场景变化时间点
    let null_dev = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };
    let filter_v = format!("select=gt(scene\\,{threshold}),showinfo");

    let output = apply_no_window(
        Command::new(ffmpeg_bin)
            .args([
                "-i",
                &video_path.to_string_lossy(),
                "-vf",
                &filter_v,
                "-vsync",
                "vfr",
                "-f",
                "null",
                "-y",
                null_dev,
            ]),
    )
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::piped())
    .output()
    .map_err(|_| AppError::MissingDependency("FFmpeg 执行失败".into()))?;

    // 从 stderr 解析 pts_time
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut pts_times: Vec<f64> = Vec::new();
    for line in stderr.lines() {
        if let Some(idx) = line.find("pts_time:") {
            let rest = &line[idx + 9..]; // skip "pts_time:"
            if let Some(end) = rest.find(|c: char| !c.is_ascii_digit() && c != '.') {
                if let Ok(val) = rest[..end].parse::<f64>() {
                    pts_times.push(val);
                }
            } else if let Ok(val) = rest.parse::<f64>() {
                pts_times.push(val);
            }
        }
    }

    if pts_times.is_empty() {
        return Ok(vec![]); // 无场景变化
    }

    // 去重（min_gap）
    let mut deduped: Vec<f64> = Vec::new();
    let mut last_ts = -min_gap - 1.0;
    for pts in pts_times {
        if pts - last_ts < min_gap {
            continue;
        }
        deduped.push(pts);
        last_ts = pts;
    }

    let timestamps: Vec<f64> = deduped.into_iter().take(max_frames).collect();

    // Pass 2: 在每个时间点截图
    let mut keyframes = Vec::new();
    for (index, ts) in timestamps.iter().enumerate() {
        let idx = index + 1;
        let safe_tag = format!("{:.1}s", ts).replace('.', "_");
        let output = output_dir.join(format!("scene_{idx:04}_{safe_tag}.png"));

        let status = apply_no_window(
            Command::new(ffmpeg_bin)
                .args([
                    "-ss",
                    &format!("{:.3}", ts),
                    "-i",
                    &video_path.to_string_lossy(),
                    "-frames:v",
                    "1",
                    "-q:v",
                    "2",
                    "-y",
                    &output.to_string_lossy(),
                ]),
        )
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| AppError::MissingDependency("FFmpeg 执行失败".into()))?;

        if status.success() && output.exists() {
            keyframes.push(Keyframe {
                file: output
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                timestamp_seconds: *ts,
                timestamp_label: timestamp_label(*ts),
                trigger: "scene".to_string(),
            });
        }
    }

    Ok(keyframes)
}

// ============================================================
// 公开入口
// ============================================================

/// 执行关键帧提取（Tauri 命令，替代 extract_keyframes.py）
#[tauri::command]
pub async fn extract_keyframes(
    video_path: String,
    output_dir: String,
    interval: u32,
    max_frames: u32,
    mode: String,
) -> Result<KeyframeResult, AppError> {
    let video = PathBuf::from(&video_path);
    let out_dir = PathBuf::from(&output_dir);

    if !video.exists() {
        return Err(AppError::Other(format!(
            "视频文件不存在: {video_path}"
        )));
    }

    let ffmpeg_bin = resolve_ffmpeg_binary("ffmpeg")
        .ok_or_else(|| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;

    log::debug!(
        target: "agent",
        "[media] phase=keyframes_start video={video_path} mode={mode} interval={interval} max_frames={max_frames}"
    );

    let frames_dir = out_dir.join("frames");
    std::fs::create_dir_all(&frames_dir).map_err(AppError::Io)?;

    let mut all_keyframes: Vec<Keyframe> = Vec::new();

    if mode == "interval" || mode == "both" {
        let interval_frames =
            extract_interval_frames(&video, &frames_dir, interval, max_frames, &ffmpeg_bin)?;
        all_keyframes.extend(interval_frames);
    }

    if mode == "scene" || mode == "both" {
        let scene_max = if mode == "scene" {
            max_frames as usize
        } else {
            (max_frames as usize / 3).max(5)
        };
        let scene_frames = extract_scene_frames(
            &video,
            &frames_dir,
            scene_max,
            &ffmpeg_bin,
            0.25,
            3.0,
            120.0,
        )?;
        all_keyframes.extend(scene_frames);
    }

    // 去重 + 按时间排序 + 限制数量
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    all_keyframes.sort_by(|a, b| {
        a.timestamp_seconds
            .partial_cmp(&b.timestamp_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let unique: Vec<Keyframe> = all_keyframes
        .into_iter()
        .filter(|kf| seen.insert(kf.file.clone()))
        .take(max_frames as usize)
        .collect();

    // 写入 keyframes.json
    let index_path = frames_dir.join("keyframes.json");
    let index_json = serde_json::to_string_pretty(&unique).map_err(AppError::Json)?;
    std::fs::write(&index_path, index_json).map_err(AppError::Io)?;

    let keyframe_count = unique.len();
    log::debug!(
        target: "agent",
        "[media] phase=keyframes_done count={keyframe_count}"
    );

    Ok(KeyframeResult {
        result: KeyframeData {
            video_path,
            output_dir,
            mode,
            interval,
            max_frames,
            keyframes: unique,
        },
    })
}

/// pipeline 内部使用的关键帧提取（支持引导时间点，用于 Agent 工具链路）
/// 只使用 scene 模式（字幕引导时间点 + 场景变化检测），不使用固定间隔
pub fn extract_keyframes_guided(
    video: &PathBuf,
    output_dir: &PathBuf,
    guided_timestamps: Option<&Path>,
) -> Result<PathBuf, AppError> {
    if !video.exists() {
        return Err(AppError::Other("视频文件不存在".into()));
    }

    let ffmpeg_bin = resolve_ffmpeg_binary("ffmpeg")
        .ok_or_else(|| AppError::MissingDependency("FFmpeg 未安装或不在 PATH".into()))?;

    log::debug!(
        target: "agent",
        "[media] phase=keyframes_guided_start video={}",
        video.display()
    );

    let frames_dir = output_dir.join("frames");
    std::fs::create_dir_all(&frames_dir).map_err(AppError::Io)?;

    let mut all_keyframes: Vec<Keyframe> = Vec::new();

    // 加载引导时间点
    let guided = load_guided_timestamps(guided_timestamps);
    if !guided.is_empty() {
        let guided_frames = extract_guided_frames(video, &frames_dir, &guided, 40, &ffmpeg_bin)?;
        if let Some(ts_path) = guided_timestamps {
            log::debug!(
                target: "agent",
                "[media] keyframes with guided timestamps: {} ({} timestamps, {} frames)",
                ts_path.display(),
                guided.len(),
                guided_frames.len()
            );
        }
        all_keyframes.extend(guided_frames);
    }

    // 场景变化检测
    let scene_frames =
        extract_scene_frames(video, &frames_dir, 40, &ffmpeg_bin, 0.25, 3.0, 120.0)?;
    all_keyframes.extend(scene_frames);

    // 去重 + 排序 + 限制
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    all_keyframes.sort_by(|a, b| {
        a.timestamp_seconds
            .partial_cmp(&b.timestamp_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let unique: Vec<&Keyframe> = all_keyframes
        .iter()
        .filter(|kf| seen.insert(kf.file.clone()))
        .take(40)
        .collect();

    // 写入 keyframes.json
    let index_path = frames_dir.join("keyframes.json");
    let index_json = serde_json::to_string_pretty(&unique).map_err(AppError::Json)?;
    std::fs::write(&index_path, index_json).map_err(AppError::Io)?;

    log::debug!(
        target: "agent",
        "[media] phase=keyframes_guided_done count={}",
        unique.len()
    );

    Ok(output_dir.clone())
}

// ============================================================
// Windows 子进程无窗口
// ============================================================

fn apply_no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
