// ============================================================
// Vision 视觉管线 — 字幕分析 + 截图审查 + 教程检测
// 全部通过 DeepSeek V4 Vision API 完成
// 提示词外置到 prompts/vision/，由 PromptManager 渲染
// ============================================================

use super::deepseek::{encode_image_to_data_url, vision_complete};
use super::prompt_manager::PromptManager;
use super::types::{
    AiTask, FrameReview, GuidedTimestamp, ReviewedFrame, ScreenshotReviewConfig,
    ScreenshotReviewResult, TutorialDetectionResult, VisionContentBlock, VisionMessage,
    VisionRequest,
};
use crate::commands::ai::engine::read_deepseek_key;
use crate::error::AppError;
use std::path::PathBuf;

// ============================================================
// 步骤 4.5: 字幕分析 → 推荐截图时间点
// 纯文本任务，不需要视觉
// ============================================================

/// 分析字幕 SRT 文本，识别画面价值高的时刻
pub async fn analyze_subtitle(
    subtitle_srt: &str,
    video_duration_seconds: f64,
    model: Option<&str>,
) -> Result<Vec<GuidedTimestamp>, AppError> {
    let api_key = read_deepseek_key()?;
    let pm = PromptManager::new()?;
    let system_prompt = pm.render(
        "vision/subtitle",
        minijinja::context! {
            video_duration_seconds => video_duration_seconds.round() as u64,
            video_duration_minutes => (video_duration_seconds / 60.0).round() as u64,
        },
    )?;

    let request = VisionRequest {
        task: AiTask::SubtitleAnalysis,
        messages: vec![VisionMessage {
            role: "user".into(),
            content: vec![VisionContentBlock::Text {
                text: subtitle_srt.to_string(),
            }],
        }],
        system_prompt,
        max_tokens: 4096,
        model_override: model.map(|s| s.to_string()),
    };

    let response = vision_complete(&request, &api_key).await?;

    // 清理可能的 markdown 代码块包裹
    let json_str = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let timestamps: Vec<GuidedTimestamp> =
        serde_json::from_str(json_str).map_err(|e| AppError::Ai {
            kind: "invalid_response".into(),
            message: format!("字幕分析 JSON 解析失败: {e}\n原始响应: {json_str}"),
        })?;

    log::debug!(
        target: "agent",
        "[vision] phase=subtitle_analysis timestamps={} srt_chars={}",
        timestamps.len(),
        subtitle_srt.len()
    );

    Ok(timestamps)
}

// ============================================================
// 步骤 7.1: 截图审查
// ============================================================

/// 审查所有候选截图，返回选中的截图列表 + 审查表
pub async fn review_keyframes(
    keyframes_json: &serde_json::Value,
    subtitle_srt: &str,
    frames_dir: &std::path::Path,
    config: &ScreenshotReviewConfig,
) -> Result<ScreenshotReviewResult, AppError> {
    if !config.enabled {
        return Ok(ScreenshotReviewResult {
            total: 0,
            selected: vec![],
            skipped: 0,
            review_table: String::new(),
        });
    }

    let frames: Vec<serde_json::Value> = match keyframes_json {
        serde_json::Value::Array(arr) => arr.clone(),
        _ => {
            return Ok(ScreenshotReviewResult {
                total: 0,
                selected: vec![],
                skipped: 0,
                review_table: "⚠️ keyframes.json 格式异常，跳过审查".into(),
            });
        }
    };

    let total = frames.len();
    if total == 0 {
        return Ok(ScreenshotReviewResult {
            total: 0,
            selected: vec![],
            skipped: 0,
            review_table: "本视频无候选截图".into(),
        });
    }

    let api_key = read_deepseek_key()?;
    let pm = PromptManager::new()?;

    // 按模式决定审查策略（pm 复用，避免循环内重复扫描模板目录）
    let reviews = match config.mode.as_str() {
        "batch" => review_batch(&pm, &frames, subtitle_srt, frames_dir, &api_key).await?,
        "single" => {
            review_single(&pm, &frames, subtitle_srt, frames_dir, &api_key, config).await?
        }
        _ => review_hybrid(&pm, &frames, subtitle_srt, frames_dir, &api_key, config).await?,
    };

    // 去重 + 按分数筛选
    let mut selected: Vec<ReviewedFrame> = reviews
        .into_iter()
        .enumerate()
        .filter_map(|(i, review)| {
            if review.info_score < config.min_score {
                return None;
            }
            // 去重: similarity > 0.85 且分数不高时跳过
            if i > 0 && review.similarity_vs_prev > 0.85 {
                return None;
            }
            let frame = &frames[i];
            Some(ReviewedFrame {
                file: frame["file"].as_str().unwrap_or("?").to_string(),
                timestamp_seconds: frame["timestamp_seconds"].as_f64().unwrap_or(0.0),
                timestamp_label: frame["timestamp_label"].as_str().unwrap_or("?").to_string(),
                trigger: frame["trigger"].as_str().unwrap_or("scene").to_string(),
                review,
            })
        })
        .collect();

    // 按分数降序排列，取前 N 张
    selected.sort_by(|a, b| b.review.info_score.cmp(&a.review.info_score));
    if selected.len() > config.max_selected {
        selected.truncate(config.max_selected);
    }
    let skipped = total - selected.len();

    // 生成审查表 Markdown
    let review_table = build_review_table(total, &selected, skipped);

    log::debug!(
        target: "agent",
        "[vision] phase=screenshot_review total={total} selected={} skipped={skipped}",
        selected.len()
    );

    Ok(ScreenshotReviewResult {
        total,
        selected,
        skipped,
        review_table,
    })
}

/// 批量审查（一次提交所有截图）
async fn review_batch(
    pm: &PromptManager,
    frames: &[serde_json::Value],
    subtitle_srt: &str,
    frames_dir: &std::path::Path,
    api_key: &str,
) -> Result<Vec<FrameReview>, AppError> {
    let mut content_blocks: Vec<VisionContentBlock> = vec![VisionContentBlock::Text {
        text: build_batch_review_prompt(pm, frames, subtitle_srt)?,
    }];

    // 添加所有截图
    for frame in frames {
        let file = frame["file"].as_str().unwrap_or("");
        let img_path = frames_dir.join(file);
        if img_path.exists() {
            match encode_image_to_data_url(&img_path) {
                Ok(data_url) => {
                    content_blocks.push(VisionContentBlock::ImageUrl {
                        image_url: super::types::ImageUrl {
                            url: data_url,
                            detail: Some("low".into()),
                        },
                    });
                }
                Err(e) => {
                    log::warn!("[vision] 截图编码失败: {file}: {e}");
                }
            }
        }
    }

    let request = VisionRequest {
        task: AiTask::ScreenshotReview,
        messages: vec![VisionMessage {
            role: "user".into(),
            content: content_blocks,
        }],
        system_prompt: build_review_system_prompt(pm)?,
        max_tokens: 8192,
        model_override: None,
    };

    let response = vision_complete(&request, api_key).await?;
    parse_review_response(&response)
}

/// 逐张审查
async fn review_single(
    pm: &PromptManager,
    frames: &[serde_json::Value],
    subtitle_srt: &str,
    frames_dir: &std::path::Path,
    api_key: &str,
    config: &ScreenshotReviewConfig,
) -> Result<Vec<FrameReview>, AppError> {
    let max_frames = config.max_review_frames.min(frames.len());
    let frames_to_review = &frames[..max_frames];

    let mut results: Vec<FrameReview> = Vec::with_capacity(frames_to_review.len());

    for (i, frame) in frames_to_review.iter().enumerate() {
        let file = frame["file"].as_str().unwrap_or("");
        let img_path = frames_dir.join(file);
        if !img_path.exists() {
            results.push(FrameReview {
                type_tag: "NO_INFO".into(),
                info_score: 0,
                similarity_vs_prev: 0.0,
                embed_section: "—".into(),
                reason: "截图文件缺失".into(),
            });
            continue;
        }

        let data_url = match encode_image_to_data_url(&img_path) {
            Ok(url) => url,
            Err(_) => {
                results.push(FrameReview {
                    type_tag: "NO_INFO".into(),
                    info_score: 0,
                    similarity_vs_prev: 0.0,
                    embed_section: "—".into(),
                    reason: "截图编码失败".into(),
                });
                continue;
            }
        };

        let timestamp = frame["timestamp_seconds"].as_f64().unwrap_or(0.0);
        let prev_type = if i > 0 {
            results[i - 1].type_tag.as_str()
        } else {
            ""
        };

        let request = VisionRequest {
            task: AiTask::ScreenshotReview,
            messages: vec![VisionMessage {
                role: "user".into(),
                content: vec![
                    VisionContentBlock::Text {
                        text: build_single_review_prompt(
                            pm,
                            i + 1,
                            frames.len(),
                            timestamp,
                            subtitle_srt,
                            prev_type,
                        )?,
                    },
                    VisionContentBlock::ImageUrl {
                        image_url: super::types::ImageUrl {
                            url: data_url,
                            detail: Some("auto".into()),
                        },
                    },
                ],
            }],
            system_prompt: build_review_system_prompt(pm)?,
            max_tokens: 1024,
            model_override: Some("deepseek-v4-flash".into()),
        };

        let response = vision_complete(&request, api_key).await?;
        let review = parse_single_review(&response).unwrap_or_else(|_| FrameReview {
            type_tag: "NO_INFO".into(),
            info_score: 0,
            similarity_vs_prev: 0.0,
            embed_section: "—".into(),
            reason: "审查响应解析失败".into(),
        });
        results.push(review);
    }

    Ok(results)
}

/// 混合审查：先批量粗筛，再逐张精审
async fn review_hybrid(
    pm: &PromptManager,
    frames: &[serde_json::Value],
    subtitle_srt: &str,
    frames_dir: &std::path::Path,
    api_key: &str,
    _config: &ScreenshotReviewConfig,
) -> Result<Vec<FrameReview>, AppError> {
    // 少于 10 张直接批量
    if frames.len() <= 10 {
        return review_batch(pm, frames, subtitle_srt, frames_dir, api_key).await;
    }

    // 第一次：批量快速粗筛（用 Flash + low detail 省 token）
    log::debug!(target: "agent", "[vision] hybrid: batch pre-scan {} frames", frames.len());
    let pre_scan = review_batch(pm, frames, subtitle_srt, frames_dir, api_key).await?;

    // 找出需要精审的帧（分数在边界附近的）
    let mut refined = vec![None; frames.len()];
    for (i, review) in pre_scan.iter().enumerate() {
        if review.info_score >= 3 {
            // 高分直接通过
            refined[i] = Some(review.clone());
        } else if review.info_score == 0 {
            // 0 分直接跳过
            refined[i] = Some(review.clone());
        }
        // score=1 或 2 的需要精审
    }

    // 对边界帧逐张精审
    for (i, review) in pre_scan.iter().enumerate() {
        if refined[i].is_some() {
            continue; // 已处理
        }

        let frame = &frames[i];
        let file = frame["file"].as_str().unwrap_or("");
        let img_path = frames_dir.join(file);
        if !img_path.exists() {
            refined[i] = Some(review.clone());
            continue;
        }

        let data_url = match encode_image_to_data_url(&img_path) {
            Ok(url) => url,
            Err(_) => {
                refined[i] = Some(review.clone());
                continue;
            }
        };

        let timestamp = frame["timestamp_seconds"].as_f64().unwrap_or(0.0);
        let prev_type = if i > 0 {
            pre_scan[i - 1].type_tag.as_str()
        } else {
            "无"
        };

        let request = VisionRequest {
            task: AiTask::ScreenshotReview,
            messages: vec![VisionMessage {
                role: "user".into(),
                content: vec![
                    VisionContentBlock::Text {
                        text: pm.render(
                            "vision/review_boundary",
                            minijinja::context! {
                                timestamp_secs => timestamp.round() as u64,
                                index => i + 1,
                                total => frames.len(),
                                prev_type => prev_type,
                            },
                        )?,
                    },
                    VisionContentBlock::ImageUrl {
                        image_url: super::types::ImageUrl {
                            url: data_url,
                            detail: Some("auto".into()),
                        },
                    },
                ],
            }],
            system_prompt: build_review_system_prompt(pm)?,
            max_tokens: 1024,
            model_override: Some("deepseek-v4-flash".into()),
        };

        match vision_complete(&request, api_key).await {
            Ok(resp) => {
                let confirmed = parse_single_review(&resp).unwrap_or_else(|_| review.clone());
                refined[i] = Some(confirmed);
            }
            Err(_) => {
                refined[i] = Some(review.clone());
            }
        }
    }

    Ok(refined.into_iter().map(|r| r.unwrap()).collect())
}

// ============================================================
// 步骤 7.4: 教程模式检测
// ============================================================

/// 检测视频是否为操作型教程
pub async fn detect_tutorial_mode(
    video_title: &str,
    first_frames: &[PathBuf],
) -> Result<TutorialDetectionResult, AppError> {
    let api_key = read_deepseek_key()?;
    let pm = PromptManager::new()?;
    let user_prompt = pm.render(
        "vision/tutorial",
        minijinja::context! {
            frame_count => first_frames.len(),
            video_title => video_title,
        },
    )?;

    let mut content_blocks: Vec<VisionContentBlock> = vec![VisionContentBlock::Text {
        text: user_prompt,
    }];

    for path in first_frames.iter().take(5) {
        if path.exists() {
            match encode_image_to_data_url(path) {
                Ok(data_url) => {
                    content_blocks.push(VisionContentBlock::ImageUrl {
                        image_url: super::types::ImageUrl {
                            url: data_url,
                            detail: Some("low".into()),
                        },
                    });
                }
                Err(_) => {}
            }
        }
    }

    let request = VisionRequest {
        task: AiTask::TutorialDetection,
        messages: vec![VisionMessage {
            role: "user".into(),
            content: content_blocks,
        }],
        // system 极短，保留代码内；user prompt 外置到 vision/tutorial.md
        system_prompt: "你是一个视频内容分类器。判断视频是否为操作型教程。只输出 JSON。".into(),
        max_tokens: 512,
        model_override: Some("deepseek-v4-flash".into()),
    };

    let response = vision_complete(&request, &api_key).await?;
    let json_str = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let result: TutorialDetectionResult =
        serde_json::from_str(json_str).unwrap_or_else(|_| TutorialDetectionResult {
            is_tutorial: false,
            confidence: 0.0,
            signals: vec![],
        });

    log::debug!(
        target: "agent",
        "[vision] phase=tutorial_detection is_tutorial={} confidence={:.2}",
        result.is_tutorial,
        result.confidence
    );

    Ok(result)
}

// ============================================================
// Prompt 模板渲染（模板文件在 prompts/vision/）
// ============================================================

fn build_review_system_prompt(pm: &PromptManager) -> Result<String, AppError> {
    pm.render("vision/review_system", ())
}

fn build_single_review_prompt(
    pm: &PromptManager,
    index: usize,
    total: usize,
    timestamp_secs: f64,
    subtitle_srt: &str,
    prev_type: &str,
) -> Result<String, AppError> {
    let ts_min = (timestamp_secs / 60.0) as u32;
    let ts_sec = (timestamp_secs as u32) % 60;
    let subtitle_snippet = extract_subtitle_context(subtitle_srt, timestamp_secs);
    pm.render(
        "vision/review_single",
        minijinja::context! {
            index => index,
            total => total,
            ts_min => ts_min,
            ts_sec_str => format!("{ts_sec:02}"),
            timestamp_secs => timestamp_secs.round() as u64,
            prev_type => prev_type,
            subtitle_snippet => &subtitle_snippet,
        },
    )
}

fn build_batch_review_prompt(
    pm: &PromptManager,
    frames: &[serde_json::Value],
    subtitle_srt: &str,
) -> Result<String, AppError> {
    let frame_list: Vec<String> = frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let file = f["file"].as_str().unwrap_or("?");
            let ts = f["timestamp_seconds"].as_f64().unwrap_or(0.0);
            let label = f["timestamp_label"].as_str().unwrap_or("?");
            let trigger = f["trigger"].as_str().unwrap_or("?");
            format!("  #{i} {file} @{label} ({ts:.0}s) trigger={trigger}")
        })
        .collect();

    let subtitle_summary = if subtitle_srt.len() > 5000 {
        format!(
            "{}...(truncated, {} chars total)",
            &subtitle_srt[..5000],
            subtitle_srt.len()
        )
    } else {
        subtitle_srt.to_string()
    };

    pm.render(
        "vision/review_batch",
        minijinja::context! {
            frame_count => frames.len(),
            frame_list => frame_list.join("\n"),
            subtitle_summary => &subtitle_summary,
        },
    )
}

fn extract_subtitle_context(srt: &str, target_secs: f64) -> String {
    // 简易 SRT 解析：找时间范围内的文本
    let lines: Vec<&str> = srt.lines().collect();
    let mut context = String::new();
    let start_range = (target_secs - 15.0).max(0.0);
    let end_range = target_secs + 15.0;

    for chunk in lines.split(|l| l.trim().is_empty()) {
        if chunk.len() < 2 {
            continue;
        }
        // 找时间行 "00:01:23,456 --> 00:01:25,789"
        if let Some(time_line) = chunk.iter().find(|l| l.contains("-->")) {
            if let Some(ts) = srt_time_to_seconds(time_line) {
                if ts >= start_range && ts <= end_range {
                    let text = chunk
                        .iter()
                        .filter(|l| !l.contains("-->") && !l.trim().parse::<u32>().is_ok())
                        .map(|l| *l)
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !text.is_empty() {
                        context.push_str(&format!("[{:.0}s] {text}\n", ts));
                    }
                }
            }
        }
    }

    if context.is_empty() {
        "(无对应字幕)".to_string()
    } else {
        context
    }
}

fn srt_time_to_seconds(time_line: &str) -> Option<f64> {
    let parts: Vec<&str> = time_line.split("-->").collect();
    let start = parts.first()?.trim();
    let time_parts: Vec<&str> = start.split([':', ',']).collect();
    if time_parts.len() >= 4 {
        let h: f64 = time_parts[0].parse().ok()?;
        let m: f64 = time_parts[1].parse().ok()?;
        let s: f64 = time_parts[2].parse().ok()?;
        let ms: f64 = time_parts[3].parse().ok()?;
        Some(h * 3600.0 + m * 60.0 + s + ms / 1000.0)
    } else {
        None
    }
}

fn parse_review_response(response: &str) -> Result<Vec<FrameReview>, AppError> {
    let text = response.trim();
    let mut reviews = Vec::new();

    // 尝试按行解析（每行一个 JSON 对象）
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "[" || trimmed == "]" {
            continue;
        }
        // 去掉行尾逗号
        let json_str = trimmed.trim_end_matches(',').trim();
        if let Ok(review) = serde_json::from_str::<FrameReview>(json_str) {
            reviews.push(review);
        }
    }

    if reviews.is_empty() {
        // 尝试作为 JSON 数组解析
        let cleaned = text
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        if let Ok(arr) = serde_json::from_str::<Vec<FrameReview>>(cleaned) {
            reviews = arr;
        }
    }

    if reviews.is_empty() {
        return Err(AppError::Ai {
            kind: "invalid_response".into(),
            message: format!("无法解析截图审查响应: {text}"),
        });
    }

    Ok(reviews)
}

fn parse_single_review(response: &str) -> Result<FrameReview, AppError> {
    let text = response.trim();
    let json_str = text
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<FrameReview>(json_str).map_err(|e| AppError::Ai {
        kind: "invalid_response".into(),
        message: format!("审查结果 JSON 解析失败: {e}\n原始: {json_str}"),
    })
}

fn build_review_table(total: usize, selected: &[ReviewedFrame], skipped: usize) -> String {
    let mut table = format!(
        "## 📸 截图审查记录（共 {total} 张，选中 {} 张，跳过 {skipped} 张）\n\n\
         | # | 时间 | 来源 | 类型 | 分数 | 决策 | 嵌入位置 |\n\
         |---|------|------|------|------|------|----------|\n",
        selected.len()
    );

    // 这里只能列出选中的（没有完整的全部审查数据），简化输出
    for (i, f) in selected.iter().enumerate() {
        let trigger_emoji = match f.trigger.as_str() {
            "guided" => "🎯引导",
            "scene" => "🔍场景",
            "gap" => "⏱️保底",
            _ => "🔍场景",
        };
        table.push_str(&format!(
            "| {} | {} | {} | {} | {} | ✅ | {} |\n",
            i + 1,
            f.timestamp_label,
            trigger_emoji,
            f.review.type_tag,
            f.review.info_score,
            f.review.embed_section,
        ));
    }

    table
}
