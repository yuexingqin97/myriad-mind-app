// ============================================================
// 媒体处理 — FFmpeg 直连，替代 extract_keyframes.py
// ============================================================
// 旧方案：Python 脚本中转 (scripts/extract_keyframes.py)
//          痛点：多一次子进程开销，行为不如 Rust 直接可控
// 新方案：Rust std::process::Command 直连 FFmpeg
//          收益：省子进程开销、日志统一、错误处理更精细
//
// 保持与旧版 extract_keyframes.py 完全一致的行为：
//   - interval 模式: `ffmpeg -vf fps=1/N`
//   - scene 模式: 两遍法 (detect pts_time via select+showinfo, 再逐帧截图)
//   - both 模式: interval + scene 合并去重
//   - guided 模式: 逐时间点 ffmpeg -ss T 截图
//   - 输出 keyframes.json 索引文件
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---- 数据结构（与 extract_keyframes.py 的 JSON 输出一致）----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeInfo {
    pub file: String,
    pub timestamp_seconds: f64,
    pub timestamp_label: String,
    #[serde(default = "default_trigger")]
    pub trigger: String,
}

fn default_trigger() -> String {
    "interval".into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyframeResult {
    pub video_path: String,
    pub output_dir: String,
    pub mode: String,
    pub interval: u32,
    pub max_frames: u32,
    pub keyframes: Vec<KeyframeInfo>,
}

/// 关键帧提取（Rust FFmpeg 直连实现）
///
/// 对应原 scripts/extract_keyframes.py 的全部功能。
/// 参数与旧版 Tauri 命令完全兼容。
///
/// 返回 `KeyframeResult`，其 JSON 结构与旧版 Python 脚本输出一致。
#[tauri::command]
pub async fn extract_keyframes(
    video_path: String,
    output_dir: String,
    interval: u32,
    max_frames: u32,
    mode: String,
) -> Result<KeyframeResult, AppError> {
    let video = Path::new(&video_path);
    let output = Path::new(&output_dir);

    if !video.exists() {
        return Err(AppError::Other(format!("视频文件不存在: {video_path}")));
    }

    extract_keyframes_direct(video, output, &mode, interval, max_frames, None)
}

/// 关键帧提取（内部函数，pipeline 直接调用）
///
/// 相比 Tauri 命令版，额外支持 `guided_timestamps` 参数。
pub(crate) fn extract_keyframes_direct(
    video: &Path,
    output_dir: &Path,
    mode: &str,
    interval: u32,
    max_frames: u32,
    guided_timestamps: Option<&Path>,
) -> Result<KeyframeResult, AppError> {
    let ffmpeg_bin = find_ffmpeg()?;
    let frames_dir = output_dir.join("frames");
    std::fs::create_dir_all(&frames_dir)?;

    let mut all_keyframes: Vec<KeyframeInfo> = Vec::new();

    // ---- 引导时间点模式 ----
    if let Some(ts_path) = guided_timestamps {
        if ts_path.exists() {
            let guided = load_guided_timestamps(ts_path)?;
            log::debug!(
                target: "agent",
                "[media] guided timestamps count={}",
                guided.len()
            );
            let guided_frames = extract_guided_frames(
                video, &frames_dir, &guided, max_frames as usize, &ffmpeg_bin,
            )?;
            all_keyframes.extend(guided_frames);
        }
    }

    // ---- interval 模式 ----
    if mode == "interval" || mode == "both" {
        let interval_frames = extract_interval_frames(
            video, &frames_dir, interval, max_frames, &ffmpeg_bin,
        )?;
        all_keyframes.extend(interval_frames);
    }

    // ---- scene 模式 ----
    if mode == "scene" || mode == "both" {
        let scene_max = if mode == "scene" {
            max_frames
        } else {
            std::cmp::max(max_frames / 3, 5)
        };
        let scene_frames = extract_scene_frames(
            video, &frames_dir, scene_max, &ffmpeg_bin, 0.12, 3.0, 120.0,
        )?;
        all_keyframes.extend(scene_frames);
    }

    // ---- 去重 + 排序 ----
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut unique: Vec<KeyframeInfo> = Vec::new();
    all_keyframes.sort_by(|a, b| {
        a.timestamp_seconds
            .partial_cmp(&b.timestamp_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for kf in all_keyframes {
        if seen.insert(kf.file.clone()) {
            unique.push(kf);
        }
    }

    // 限制到 max_frames
    unique.truncate(max_frames as usize);

    // ---- 写入 keyframes.json 索引 ----
    let index_path = frames_dir.join("keyframes.json");
    let index_json = serde_json::to_string_pretty(&unique)?;
    std::fs::write(&index_path, &index_json)?;

    log::debug!(
        target: "agent",
        "[media] keyframes done mode={mode} total={} written={}",
        unique.len(),
        index_path.display()
    );

    Ok(KeyframeResult {
        video_path: video.to_string_lossy().to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
        mode: mode.to_string(),
        interval,
        max_frames,
        keyframes: unique,
    })
}

// ============================================================
// 内部辅助函数
// ============================================================

/// 查找 ffmpeg 可执行文件
fn find_ffmpeg() -> Result<String, AppError> {
    // 优先 PATH
    if let Some(path) = find_in_path("ffmpeg") {
        return Ok(path);
    }

    // Windows: 检查 winget 安装目录
    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_base = PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if winget_base.exists() {
                if let Some(path) = find_ffmpeg_in_dir(&winget_base, 4) {
                    return Ok(path);
                }
            }
        }
    }

    Err(AppError::MissingDependency(
        "ffmpeg not found. Install with: winget install Gyan.FFmpeg (Windows) / brew install ffmpeg (macOS) / sudo apt install ffmpeg (Linux)"
            .into(),
    ))
}

/// 在 PATH 中搜索可执行文件
fn find_in_path(name: &str) -> Option<String> {
    let exe_name = if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    };

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(';') {
            let candidate = Path::new(dir).join(&exe_name);
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// 递归在目录中查找 ffmpeg.exe
#[cfg(target_os = "windows")]
fn find_ffmpeg_in_dir(base: &Path, max_depth: u32) -> Option<String> {
    if max_depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(base).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase().contains("ffmpeg"))
                .unwrap_or(false)
            {
                let bin_ffmpeg = path.join("bin").join("ffmpeg.exe");
                if bin_ffmpeg.exists() {
                    return Some(bin_ffmpeg.to_string_lossy().to_string());
                }
                // 递归子目录
                if let Some(found) = find_ffmpeg_in_dir(&path, max_depth - 1) {
                    return Some(found);
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn find_ffmpeg_in_dir(_base: &Path, _max_depth: u32) -> Option<String> {
    None
}

/// 读取引导时间点 JSON
fn load_guided_timestamps(path: &Path) -> Result<Vec<(f64, String)>, AppError> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        AppError::Other(format!("读取引导时间点文件失败: {e}"))
    })?;

    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        AppError::Other(format!("引导时间点 JSON 解析失败: {e}"))
    })?;

    let items = if let Some(arr) = value.get("timestamps").and_then(|v| v.as_array()) {
        arr.clone()
    } else if let Some(arr) = value.as_array() {
        arr.clone()
    } else {
        return Ok(Vec::new());
    };

    let mut result: Vec<(f64, String)> = Vec::new();
    for item in &items {
        if let Some(n) = item.as_f64() {
            result.push((n, "AI推荐".into()));
        } else if let Some(n) = item.as_i64() {
            result.push((n as f64, "AI推荐".into()));
        } else if let Some(obj) = item.as_object() {
            let ts = obj
                .get("ts")
                .or_else(|| obj.get("timestamp"))
                .or_else(|| obj.get("timestamp_seconds"))
                .and_then(|v| v.as_f64())
                .or_else(|| {
                    obj.get("ts")
                        .or_else(|| obj.get("timestamp"))
                        .or_else(|| obj.get("timestamp_seconds"))
                        .and_then(|v| v.as_i64())
                        .map(|n| n as f64)
                });
            if let Some(ts) = ts {
                let reason = obj
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("AI推荐")
                    .to_string();
                result.push((ts, reason));
            }
        }
    }

    // 去重：时间点间隔 < 2s 的合并
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

/// 时间戳标签: 秒 → "02m35s" 或 "01h02m35s"
fn timestamp_label(seconds: f64) -> String {
    let total_secs = (seconds.round() as i64).max(0);
    let minutes = total_secs / 60;
    let secs = total_secs % 60;
    let hours = minutes / 60;
    let mins = minutes % 60;
    if hours > 0 {
        format!("{hours:02}h{mins:02}m{secs:02}s")
    } else {
        format!("{mins:02}m{secs:02}s")
    }
}

/// reason 转文件安全标签
fn slug_reason(reason: &str) -> String {
    let cleaned: String = reason
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if cleaned.len() > 24 { cleaned[..24].to_string() } else { cleaned }
}

/// 逐时间点截图（引导模式）
fn extract_guided_frames(
    video: &Path,
    frames_dir: &Path,
    timestamps: &[(f64, String)],
    max_frames: usize,
    ffmpeg_bin: &str,
) -> Result<Vec<KeyframeInfo>, AppError> {
    let mut keyframes: Vec<KeyframeInfo> = Vec::new();

    for (idx, (ts, reason)) in timestamps.iter().take(max_frames).enumerate() {
        let output = frames_dir.join(format!(
            "guided_{:04}_{}.png",
            idx + 1,
            slug_reason(reason)
        ));

        let status = std::process::Command::new(ffmpeg_bin)
            .args([
                "-ss", &format!("{:.3}", ts),
                "-i", &video.to_string_lossy().to_string(),
                "-frames:v", "1",
                "-q:v", "2",
                "-y",
                &output.to_string_lossy().to_string(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| AppError::Other(format!("ffmpeg 引导截图失败: {e}")))?;

        if status.success() && output.exists() {
            keyframes.push(KeyframeInfo {
                file: output
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                timestamp_seconds: *ts,
                timestamp_label: timestamp_label(*ts),
                trigger: format!("guided:{reason}"),
            });
        }
    }

    Ok(keyframes)
}

/// 固定间隔截图（interval 模式）
fn extract_interval_frames(
    video: &Path,
    frames_dir: &Path,
    interval: u32,
    max_frames: u32,
    ffmpeg_bin: &str,
) -> Result<Vec<KeyframeInfo>, AppError> {
    let pattern = frames_dir.join("frame_%04d.png");
    let fps = 1.0 / interval as f64;

    let status = std::process::Command::new(ffmpeg_bin)
        .args([
            "-i", &video.to_string_lossy().to_string(),
            "-vf", &format!("fps={:.6}", fps),
            "-frames:v", &max_frames.to_string(),
            "-q:v", "2",
            "-y",
            &pattern.to_string_lossy().to_string(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| AppError::Other(format!("ffmpeg 间隔截图失败: {e}")))?;

    if !status.success() {
        return Err(AppError::Other("ffmpeg 间隔截图返回非0退出码".into()));
    }

    // 收集生成的帧文件
    let mut keyframes: Vec<KeyframeInfo> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(frames_dir) {
        let mut pngs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .map(|e| e == "png")
                    .unwrap_or(false)
                    && p.file_stem()
                        .map(|s| s.to_string_lossy().starts_with("frame_"))
                        .unwrap_or(false)
            })
            .collect();
        pngs.sort();

        for png in pngs {
            if let Some(stem) = png.file_stem().and_then(|s| s.to_str()) {
                // frame_0001 → num=1
                if let Some(num_str) = stem.strip_prefix("frame_") {
                    if let Ok(num) = num_str.parse::<u32>() {
                        let ts = (num.saturating_sub(1)) as f64 * interval as f64;
                        keyframes.push(KeyframeInfo {
                            file: png
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            timestamp_seconds: ts,
                            timestamp_label: timestamp_label(ts),
                            trigger: "interval".into(),
                        });
                    }
                }
            }
        }
    }

    Ok(keyframes)
}

/// 场景变化检测截图（scene 模式，两遍法）
fn extract_scene_frames(
    video: &Path,
    frames_dir: &Path,
    max_frames: u32,
    ffmpeg_bin: &str,
    threshold: f64,
    min_gap: f64,
    _max_gap: f64,
) -> Result<Vec<KeyframeInfo>, AppError> {
    // ---- 第1遍: 检测场景变化 pts_time ----
    // Windows NUL, Unix /dev/null
    let null_dev = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };

    let filter_v = format!("select=gt(scene\\,{}),showinfo", threshold);

    let output = std::process::Command::new(ffmpeg_bin)
        .args([
            "-i", &video.to_string_lossy().to_string(),
            "-vf", &filter_v,
            "-vsync", "vfr",
            "-f", "null",
            "-y",
            null_dev,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| AppError::Other(format!("ffmpeg 场景检测失败: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    // 解析 pts_time 值（手写解析，避免引入 regex crate）
    let mut pts_times: Vec<f64> = Vec::new();
    for line in stderr.lines() {
        if let Some(pos) = line.find("pts_time:") {
            let after = &line[pos + "pts_time:".len()..];
            // 提取数字和小数点部分
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if let Ok(ts) = num_str.parse::<f64>() {
                pts_times.push(ts);
            }
        }
    }

    if pts_times.is_empty() {
        return Ok(Vec::new()); // 无场景变化
    }

    // 去重：min_gap 间隔
    let mut deduped: Vec<f64> = Vec::new();
    let mut last_ts = -min_gap - 1.0;
    for pts in pts_times {
        if pts - last_ts < min_gap {
            continue;
        }
        deduped.push(pts);
        last_ts = pts;
    }

    let timestamps = &deduped[..deduped.len().min(max_frames as usize)];

    // ---- 第2遍: 逐个时间点截图 ----
    let mut keyframes: Vec<KeyframeInfo> = Vec::new();
    for (idx, ts) in timestamps.iter().enumerate() {
        let safe_tag = format!("{:.1}s", ts).replace('.', "_");
        let output = frames_dir.join(format!("scene_{:04}_{}.png", idx + 1, safe_tag));

        let cmd_status = std::process::Command::new(ffmpeg_bin)
            .args([
                "-ss", &format!("{:.3}", ts),
                "-i", &video.to_string_lossy().to_string(),
                "-frames:v", "1",
                "-q:v", "2",
                "-y",
                &output.to_string_lossy().to_string(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| AppError::Other(format!("ffmpeg 场景截图失败: {e}")))?;

        if cmd_status.success() && output.exists() {
            keyframes.push(KeyframeInfo {
                file: output
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                timestamp_seconds: *ts,
                timestamp_label: timestamp_label(*ts),
                trigger: "scene".into(),
            });
        }
    }

    Ok(keyframes)
}
