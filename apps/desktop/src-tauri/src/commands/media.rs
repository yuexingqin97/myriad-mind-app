// ============================================================
// 媒体处理 — FFmpeg 直连
// 原 extract_keyframes.py 的截图逻辑迁到 Rust，直接 spawn ffmpeg。
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 关键帧提取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeResult {
    pub result: KeyframeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeData {
    pub video_path: String,
    pub output_dir: String,
    pub mode: String,
    pub interval: u32,
    pub max_frames: u32,
    pub keyframes: Vec<KeyframeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeInfo {
    pub file: String,
    pub timestamp_seconds: f64,
    pub timestamp_label: String,
    pub trigger: String,
}

const DEFAULT_INTERVAL: u32 = 30;
const DEFAULT_MAX_FRAMES: u32 = 40;
const DEFAULT_MODE: &str = "both";
const DEFAULT_SCENE_THRESHOLD: f64 = 0.25;
const DEFAULT_MIN_GAP: f64 = 3.0;
const DEFAULT_MAX_GAP: f64 = 120.0;

// ============================================================
// FFmpeg 定位
// ============================================================

/// 在 PATH 中查找可执行文件
fn which(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .filter_map(|dir| {
                let full = dir.join(&exe);
                if full.is_file() {
                    Some(full)
                } else {
                    None
                }
            })
            .next()
    })
}

/// 递归查找目录下的 ffmpeg.exe（用于 WinGet 安装路径兜底）
#[cfg(target_os = "windows")]
fn find_ffmpeg_under_winget(base: &Path) -> Option<PathBuf> {
    fn walk(dir: &Path, max_depth: usize) -> Option<PathBuf> {
        if max_depth == 0 {
            return None;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map_or(false, |n| n.eq_ignore_ascii_case("ffmpeg.exe"))
                    {
                        return Some(path);
                    }
                } else if path.is_dir() {
                    if let Some(found) = walk(&path, max_depth - 1) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    // 优先直接看 pkg/bin
    let bin = base.join("bin").join("ffmpeg.exe");
    if bin.is_file() {
        return Some(bin);
    }

    walk(base, 4)
}

/// 定位 ffmpeg 可执行文件
/// 顺序：PATH → WinGet 常见安装路径（Windows）
fn find_ffmpeg() -> Result<PathBuf, AppError> {
    if let Some(path) = which("ffmpeg") {
        return Ok(path);
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let winget_base = PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if let Ok(entries) = std::fs::read_dir(&winget_base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir()
                        && path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map_or(false, |n| n.to_lowercase().contains("ffmpeg"))
                    {
                        if let Some(ffmpeg) = find_ffmpeg_under_winget(&path) {
                            return Ok(ffmpeg);
                        }
                    }
                }
            }
        }
    }

    Err(AppError::MissingDependency(
        "ffmpeg 未安装或不在 PATH。建议：winget install Gyan.FFmpeg".into(),
    ))
}

// ============================================================
// 工具函数
// ============================================================

/// 把秒数格式化为与 Python 脚本一致的时间标签
fn timestamp_label(seconds: f64) -> String {
    let total = seconds.round() as i64;
    let total = total.max(0);
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours:02}h{minutes:02}m{secs:02}s")
    } else {
        format!("{minutes:02}m{secs:02}s")
    }
}

/// 把 reason 转成文件名安全的 slug
fn slug_reason(reason: &str) -> String {
    let parts: Vec<&str> = reason.split_whitespace().collect();
    let joined = parts.join("_");
    let cleaned: String = joined
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || is_cjk(*c))
        .collect();
    let trimmed: String = cleaned.chars().take(24).collect();
    if trimmed.is_empty() {
        "guided".into()
    } else {
        trimmed
    }
}

fn is_cjk(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

/// 解析引导时间戳 JSON
/// 支持对象数组 `{ts, reason}` 或纯数字数组；也支持顶层带 `timestamps` 字段的对象。
fn load_guided_timestamps(path: Option<&Path>) -> Result<Vec<(f64, String)>, AppError> {
    let path = match path {
        Some(p) if p.exists() => p,
        _ => return Ok(Vec::new()),
    };

    let text = std::fs::read_to_string(path)?;
    let raw: serde_json::Value = serde_json::from_str(&text)?;

    let items = match raw {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(mut m) => m
            .remove("timestamps")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    let mut result = Vec::new();
    for item in items {
        match item {
            serde_json::Value::Number(n) => {
                if let Some(ts) = n.as_f64() {
                    result.push((ts, "AI推荐".into()));
                }
            }
            serde_json::Value::Object(m) => {
                let ts = m
                    .get("ts")
                    .or_else(|| m.get("timestamp"))
                    .or_else(|| m.get("timestamp_seconds"))
                    .and_then(|v| v.as_f64());
                let reason = m
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("AI推荐")
                    .to_string();
                if let Some(ts) = ts {
                    result.push((ts, reason));
                }
            }
            _ => {}
        }
    }

    result.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut deduped: Vec<(f64, String)> = Vec::new();
    for (ts, reason) in result {
        if ts < 0.0 {
            continue;
        }
        if let Some(last) = deduped.last() {
            if (ts - last.0).abs() < 2.0 {
                continue;
            }
        }
        deduped.push((ts, reason));
    }

    Ok(deduped)
}

// ============================================================
// 帧提取模式
// ============================================================

/// 引导帧提取：在指定时间点各截一张图
fn extract_guided_frames(
    ffmpeg: &Path,
    video: &Path,
    frames_dir: &Path,
    timestamps: &[(f64, String)],
    max_frames: usize,
) -> Result<Vec<KeyframeInfo>, AppError> {
    std::fs::create_dir_all(frames_dir)?;
    let mut keyframes = Vec::new();

    for (index, (ts, reason)) in timestamps.iter().take(max_frames).enumerate() {
        let index = index + 1;
        let output = frames_dir.join(format!("guided_{index:04}_{}.png", slug_reason(reason)));
        let status = Command::new(ffmpeg)
            .arg("-ss")
            .arg(format!("{ts:.3}"))
            .arg("-i")
            .arg(video)
            .args(["-frames:v", "1", "-q:v", "2", "-y"])
            .arg(&output)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(AppError::Other(format!(
                "ffmpeg 引导截图失败 @ {ts:.3}s"
            )));
        }

        if output.exists() {
            keyframes.push(KeyframeInfo {
                file: output
                    .file_name()
                    .expect("文件名存在")
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

/// 固定间隔帧提取
fn extract_interval_frames(
    ffmpeg: &Path,
    video: &Path,
    frames_dir: &Path,
    interval: u32,
    max_frames: u32,
) -> Result<Vec<KeyframeInfo>, AppError> {
    std::fs::create_dir_all(frames_dir)?;

    let fps = 1.0 / f64::from(interval);
    let filter = format!("fps={fps:.6}");
    let pattern = frames_dir.join("frame_%04d.png");

    let status = Command::new(ffmpeg)
        .arg("-i")
        .arg(video)
        .arg("-vf")
        .arg(filter)
        .args([
            "-frames:v",
            &max_frames.to_string(),
            "-q:v",
            "2",
            "-y",
        ])
        .arg(&pattern)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    if !status.success() {
        return Err(AppError::Other("ffmpeg 间隔截图失败".into()));
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(frames_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map_or(false, |e| e.eq_ignore_ascii_case("png"))
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .map_or(false, |s| s.starts_with("frame_"))
        })
        .collect();
    paths.sort();

    let mut keyframes = Vec::new();
    for path in paths {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let num: u32 = stem
            .split('_')
            .nth(1)
            .and_then(|n| n.parse().ok())
            .unwrap_or(1);
        let ts = f64::from((num.saturating_sub(1)) * interval);
        keyframes.push(KeyframeInfo {
            file: path
                .file_name()
                .expect("文件名存在")
                .to_string_lossy()
                .to_string(),
            timestamp_seconds: ts,
            timestamp_label: timestamp_label(ts),
            trigger: "interval".into(),
        });
    }

    Ok(keyframes)
}

/// 场景切换帧提取
fn extract_scene_frames(
    ffmpeg: &Path,
    video: &Path,
    frames_dir: &Path,
    max_frames: usize,
    threshold: f64,
    min_gap: f64,
) -> Result<Vec<KeyframeInfo>, AppError> {
    std::fs::create_dir_all(frames_dir)?;

    // ---- Pass 1: 检测场景切换时间点 ----
    let null_dev = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };
    let filter = format!("select=gt(scene\\,{threshold})");
    let output = Command::new(ffmpeg)
        .arg("-i")
        .arg(video)
        .arg("-vf")
        .arg(&filter)
        .args(["-vsync", "vfr", "-f", "null", "-y"])
        .arg(null_dev)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut pts_times: Vec<f64> = Vec::new();
    for line in stderr.lines() {
        // 匹配 "pts_time:123.456"
        if let Some(pos) = line.find("pts_time:") {
            let rest = &line[pos + "pts_time:".len()..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if let Ok(ts) = num_str.parse::<f64>() {
                pts_times.push(ts);
            }
        }
    }

    // 去重：最小间隔 min_gap
    let mut deduped: Vec<f64> = Vec::new();
    let mut last_ts = -min_gap - 1.0;
    for ts in pts_times {
        if ts - last_ts < min_gap {
            continue;
        }
        deduped.push(ts);
        last_ts = ts;
    }
    deduped.truncate(max_frames);

    // ---- Pass 2: 逐时间戳截图 ----
    let mut keyframes = Vec::new();
    for (index, ts) in deduped.iter().enumerate() {
        let index = index + 1;
        let safe_tag = format!("{ts:.1}s").replace('.', "_");
        let output = frames_dir.join(format!("scene_{index:04}_{safe_tag}.png"));
        let status = Command::new(ffmpeg)
            .arg("-ss")
            .arg(format!("{ts:.3}"))
            .arg("-i")
            .arg(video)
            .args(["-frames:v", "1", "-q:v", "2", "-y"])
            .arg(&output)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            return Err(AppError::Other(format!("ffmpeg 场景截图失败 @ {ts:.3}s")));
        }

        if output.exists() {
            keyframes.push(KeyframeInfo {
                file: output
                    .file_name()
                    .expect("文件名存在")
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

// ============================================================
// 主入口
// ============================================================

fn extract_keyframes_impl(
    video_path: &Path,
    output_dir: &Path,
    interval: u32,
    max_frames: u32,
    mode: &str,
    timestamps_path: Option<&Path>,
    scene_threshold: f64,
    min_gap: f64,
    max_gap: f64,
) -> Result<KeyframeResult, AppError> {
    let _ = max_gap; // 与原脚本签名兼容，当前实现按 min_gap 去重已足够

    if !video_path.exists() {
        return Err(AppError::Other(format!(
            "视频文件不存在: {}",
            video_path.display()
        )));
    }

    let ffmpeg = find_ffmpeg()?;
    let frames_dir = output_dir.join("frames");
    std::fs::create_dir_all(&frames_dir)?;

    log::debug!(
        target: "agent",
        "[media] phase=extract_keyframes video={} mode={mode} interval={interval} max_frames={max_frames}",
        video_path.display()
    );

    let mut all_keyframes: Vec<KeyframeInfo> = Vec::new();

    let guided_timestamps = load_guided_timestamps(timestamps_path)?;
    if !guided_timestamps.is_empty() {
        let guided = extract_guided_frames(
            &ffmpeg,
            video_path,
            &frames_dir,
            &guided_timestamps,
            max_frames as usize,
        )?;
        all_keyframes.extend(guided);
    }

    if mode == "interval" || mode == "both" {
        let interval_frames = extract_interval_frames(
            &ffmpeg,
            video_path,
            &frames_dir,
            interval,
            max_frames,
        )?;
        all_keyframes.extend(interval_frames);
    }

    if mode == "scene" || mode == "both" {
        let scene_max = if mode == "scene" {
            max_frames as usize
        } else {
            (max_frames as usize / 3).max(5)
        };
        let scene_frames = extract_scene_frames(
            &ffmpeg,
            video_path,
            &frames_dir,
            scene_max,
            scene_threshold,
            min_gap,
        )?;
        all_keyframes.extend(scene_frames);
    }

    // 按时间戳排序 + 按文件名去重 + 截断到 max_frames
    all_keyframes.sort_by(|a, b| {
        a.timestamp_seconds
            .partial_cmp(&b.timestamp_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen = HashSet::new();
    let mut unique: Vec<KeyframeInfo> = Vec::new();
    for kf in all_keyframes {
        if seen.insert(kf.file.clone()) {
            unique.push(kf);
        }
    }
    unique.truncate(max_frames as usize);

    // 写入 keyframes.json
    let index_path = frames_dir.join("keyframes.json");
    let index_data = serde_json::to_string_pretty(&unique)?;
    std::fs::write(&index_path, index_data)?;

    log::debug!(
        target: "agent",
        "[media] phase=extract_keyframes_done frames={} index={}",
        unique.len(),
        index_path.display()
    );

    Ok(KeyframeResult {
        result: KeyframeData {
            video_path: video_path.to_string_lossy().to_string(),
            output_dir: output_dir.to_string_lossy().to_string(),
            mode: mode.into(),
            interval,
            max_frames,
            keyframes: unique,
        },
    })
}

/// 受引导的截图入口（pipeline / agent tool 使用）
/// 固定使用 scene 模式 + 可选引导时间戳，返回 frames 目录的父目录（output_dir）。
pub fn extract_keyframes_guided(
    video: &Path,
    output_dir: &Path,
    guided_timestamps: Option<&Path>,
) -> Result<PathBuf, AppError> {
    tokio::task::block_in_place(|| {
        extract_keyframes_impl(
            video,
            output_dir,
            DEFAULT_INTERVAL,
            DEFAULT_MAX_FRAMES,
            "scene",
            guided_timestamps,
            DEFAULT_SCENE_THRESHOLD,
            DEFAULT_MIN_GAP,
            DEFAULT_MAX_GAP,
        )
        .map(|_| output_dir.to_path_buf())
    })
}

/// Tauri 命令：关键帧提取
/// 保留原 Python 命令的签名（含已废弃的 python_path 字段），行为完全一致。
#[tauri::command]
pub async fn extract_keyframes(
    video_path: String,
    output_dir: String,
    _python_path: String,
    interval: u32,
    max_frames: u32,
    mode: String,
) -> Result<KeyframeResult, AppError> {
    let video = PathBuf::from(video_path);
    let output = PathBuf::from(output_dir);

    let mode = if mode.trim().is_empty() {
        DEFAULT_MODE.to_string()
    } else {
        mode
    };

    tokio::task::spawn_blocking(move || {
        extract_keyframes_impl(
            &video,
            &output,
            if interval == 0 { DEFAULT_INTERVAL } else { interval },
            if max_frames == 0 { DEFAULT_MAX_FRAMES } else { max_frames },
            &mode,
            None,
            DEFAULT_SCENE_THRESHOLD,
            DEFAULT_MIN_GAP,
            DEFAULT_MAX_GAP,
        )
    })
    .await
    .map_err(|e| AppError::Other(format!("截图任务被中断: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::{extract_keyframes_impl, slug_reason, timestamp_label};

    #[test]
    fn timestamp_label_formats() {
        assert_eq!(timestamp_label(0.0), "00m00s");
        assert_eq!(timestamp_label(59.0), "00m59s");
        assert_eq!(timestamp_label(60.0), "01m00s");
        assert_eq!(timestamp_label(3661.0), "01h01m01s");
        assert_eq!(timestamp_label(183.4), "03m03s"); // 183.4 -> round 183
    }

    #[test]
    fn slug_reason_filters() {
        assert_eq!(slug_reason("AI 推荐 时间点"), "AI_推荐_时间点");
        assert_eq!(slug_reason("  hello world  "), "hello_world");
        assert_eq!(slug_reason("a!b@c#"), "abc");
        assert_eq!(slug_reason(""), "guided");
    }

    /// 用 ffmpeg 生成一段 10s 测试视频，验证 interval 模式截图 end-to-end 可运行。
    /// 需要本机已安装 ffmpeg（开发环境必备）。
    #[test]
    fn extract_keyframes_interval_smoke() {
        let tmp = std::env::temp_dir().join(format!(
            "myriad-mind-media-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let video = tmp.join("input.mp4");
        let output_dir = tmp.join("keyframes");

        // 生成 10s、1fps、1280x720 的测试视频
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-f", "lavfi",
                "-i", "testsrc=duration=10:size=1280x720:rate=1",
                "-pix_fmt", "yuv420p",
                "-y",
            ])
            .arg(&video)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("无法运行 ffmpeg 生成测试视频");
        assert!(status.success(), "ffmpeg 测试视频生成失败");

        let result = extract_keyframes_impl(
            &video,
            &output_dir,
            2,    // interval
            5,    // max_frames
            "interval",
            None, // no guided timestamps
            0.25,
            3.0,
            120.0,
        )
        .expect("截图应成功");

        assert_eq!(result.result.mode, "interval");
        assert_eq!(result.result.keyframes.len(), 5);

        let frames_dir = output_dir.join("frames");
        assert!(frames_dir.exists());
        assert!(frames_dir.join("keyframes.json").exists());

        let expected_labels = ["00m00s", "00m02s", "00m04s", "00m06s", "00m08s"];
        for (i, kf) in result.result.keyframes.iter().enumerate() {
            assert_eq!(kf.timestamp_label, expected_labels[i]);
            assert!(frames_dir.join(&kf.file).exists(), "截图文件应存在: {}", kf.file);
        }

        // 清理
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
