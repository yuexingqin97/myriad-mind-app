// ============================================================
// Vision 视觉管线 — 字幕分析 + 截图审查 + 教程检测
// 全部通过 DeepSeek V4 Vision API 完成
// ============================================================

use super::deepseek::{encode_image_to_data_url, vision_complete};
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

    let system_prompt = format!(
        "你是一个视频字幕分析器。请分析以下视频字幕，找出画面价值高的时刻（适合截图的瞬间）。\n\
        视频总时长：{:.0} 秒（约 {:.0} 分钟）\n\n\
        ## 识别信号\n\
        - 画面展示: \"看这段代码\"、\"如图所示\"、\"注意这个表格\"、\"我们来看一下\"\n\
        - 操作演示: \"点击这里\"、\"打开终端\"、\"运行一下\"、\"输入以下命令\"\n\
        - 代码相关: \"这段代码\"、\"函数定义\"、\"配置文件\"、\"这个接口\"\n\
        - PPT翻页: \"下一页\"、\"接下来看\"、\"第一个要点\"、\"这一章\"、\"总结一下\"\n\
        - 对比切换: \"对比一下\"、\"切换到\"、\"改成\"、\"前后的区别\"\n\
        - 运行效果: \"运行结果\"、\"输出是\"、\"报错了\"、\"执行效果\"\n\n\
        ## 不推荐的时刻\n\
        - 开场闲聊、个人介绍、片尾预告、过渡寒暄\n\n\
        ## 输出格式\n\
        返回 JSON 数组，每个元素包含 ts（秒数，精确到整数）和 reason（20字以内）：\n\
        [{{\"ts\": 32, \"reason\": \"PPT标题页：ECS三大核心概念\"}}, ...]\n\
        推荐 8-25 个时间点。如果视频为纯谈话无技术内容，输出空数组 []。\n\n\
        只输出 JSON 数组，不要其他内容。",
        video_duration_seconds,
        video_duration_seconds / 60.0
    );

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

    log::info!(
        "[vision] subtitle analysis: {} guided timestamps from {} chars of SRT",
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

    // 按模式决定审查策略
    let reviews = match config.mode.as_str() {
        "batch" => review_batch(&frames, subtitle_srt, frames_dir, &api_key).await?,
        "single" => review_single(&frames, subtitle_srt, frames_dir, &api_key, config).await?,
        _ => review_hybrid(&frames, subtitle_srt, frames_dir, &api_key, config).await?,
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

    log::info!(
        "[vision] screenshot review: {total} frames → {} selected, {skipped} skipped",
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
    frames: &[serde_json::Value],
    subtitle_srt: &str,
    frames_dir: &std::path::Path,
    api_key: &str,
) -> Result<Vec<FrameReview>, AppError> {
    let mut content_blocks: Vec<VisionContentBlock> = vec![VisionContentBlock::Text {
        text: build_batch_review_prompt(frames, subtitle_srt),
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
        system_prompt: build_review_system_prompt(),
        max_tokens: 8192,
        model_override: None,
    };

    let response = vision_complete(&request, api_key).await?;
    parse_review_response(&response)
}

/// 逐张审查
async fn review_single(
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
                            i + 1,
                            frames.len(),
                            timestamp,
                            subtitle_srt,
                            prev_type,
                        ),
                    },
                    VisionContentBlock::ImageUrl {
                        image_url: super::types::ImageUrl {
                            url: data_url,
                            detail: Some("auto".into()),
                        },
                    },
                ],
            }],
            system_prompt: build_review_system_prompt(),
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
    frames: &[serde_json::Value],
    subtitle_srt: &str,
    frames_dir: &std::path::Path,
    api_key: &str,
    _config: &ScreenshotReviewConfig,
) -> Result<Vec<FrameReview>, AppError> {
    // 少于 10 张直接批量
    if frames.len() <= 10 {
        return review_batch(frames, subtitle_srt, frames_dir, api_key).await;
    }

    // 第一次：批量快速粗筛（用 Flash + low detail 省 token）
    log::info!("[vision] hybrid: batch pre-scan {} frames", frames.len());
    let pre_scan = review_batch(frames, subtitle_srt, frames_dir, api_key).await?;

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

        let request = VisionRequest {
            task: AiTask::ScreenshotReview,
            messages: vec![VisionMessage {
                role: "user".into(),
                content: vec![
                    VisionContentBlock::Text {
                        text: format!(
                            "这张截图在批量粗筛中得了 2 分（边界）。请仔细确认：\n\
                            时间戳: {:.0}s\n批次索引: {}/{}\n前次类型: {}",
                            timestamp,
                            i + 1,
                            frames.len(),
                            if i > 0 {
                                pre_scan[i - 1].type_tag.as_str()
                            } else {
                                "无"
                            }
                        ),
                    },
                    VisionContentBlock::ImageUrl {
                        image_url: super::types::ImageUrl {
                            url: data_url,
                            detail: Some("auto".into()),
                        },
                    },
                ],
            }],
            system_prompt: build_review_system_prompt(),
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

    let mut content_blocks: Vec<VisionContentBlock> = vec![VisionContentBlock::Text {
        text: format!(
            "根据视频标题和前 {} 张关键帧截图，判断这是否为\"操作型教程\"。\n\n\
            视频标题: {video_title}\n\n\
            操作型教程特征：\n\
            - 标题含\"教程\"/\"入门\"/\"实战\"/\"配置\"/\"搭建\"/\"tutorial\"/\"how to\"\n\
            - 画面中有大量 IDE/终端/操作界面截图\n\
            - 内容以\"一步步跟着做\"为主\n\n\
            输出 JSON：\n\
            {{\"is_tutorial\": true/false, \"confidence\": 0.0-1.0, \"signals\": [\"标题含'教程'\", \"5张中有4张是操作界面\"]}}\n\n\
            只输出 JSON，不要其他内容。",
            first_frames.len()
        ),
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

    log::info!(
        "[vision] tutorial detection: is_tutorial={}, confidence={:.2}",
        result.is_tutorial,
        result.confidence
    );

    Ok(result)
}

// ============================================================
// Prompt 模板
// ============================================================

fn build_review_system_prompt() -> String {
    r#"你是一个视频学习笔记的截图审查员。请分析视频截图，判断其是否值得嵌入学习笔记。

## 类型标签（必须选一个）
PPT_TITLE: PPT标题页/大纲/章节分隔页 → 3分
CODE_BLOCK: 代码/配置文件/终端输出 → 3分
ARCH_DIAGRAM: 架构图/流程图/数据流图 → 3分
RUN_RESULT: 程序运行效果/错误输出 → 3分
TOOL_UI: 编辑器/IDE/工具操作界面 → 3分
DATA_TABLE: 数据表格/对比表 → 2分
SIMPLE_CHART: 简单图表 → 2分
SPLIT_SCREEN: 分屏布局(讲师+代码) → 2分
TALKING_HEAD: 说话人脸(无辅助视觉信息) → 0分
PLAIN_TEXT: 纯文字段落(字幕已覆盖) → 0分
BLACK_SCREEN: 黑屏/过渡动画 → 0分
NO_INFO: 空桌面/模糊/无关 → 0分

## 评分规则
最终得分 = 基础分 × 上下文加成
- 实质性技术内容 + 信息画面: ×1.0
- 实质性技术内容 + 静态人脸: ×0（跳过）
- 闲聊/过渡 + 任何画面: ×0.3
- 无字幕时段: ×0（跳过）

≥3分 → 必选 ✅
=2分 → 可选 ⚖️
≤1分 → 跳过 ❌

## 输出格式
每张截图输出一行 JSON（不嵌套在数组里）:
{"type_tag": "...", "info_score": 0-3, "similarity_vs_prev": 0-1, "embed_section": "...", "reason": "20字以内"}"#
        .to_string()
}

fn build_single_review_prompt(
    index: usize,
    total: usize,
    timestamp_secs: f64,
    subtitle_srt: &str,
    prev_type: &str,
) -> String {
    let ts_min = (timestamp_secs / 60.0) as u32;
    let ts_sec = (timestamp_secs as u32) % 60;
    let subtitle_snippet = extract_subtitle_context(subtitle_srt, timestamp_secs);

    format!(
        "审查第 {index}/{total} 张截图。\n\
        时间戳: {ts_min}m{ts_sec:02}s ({timestamp_secs:.0}s)\n\
        前一张类型: {prev_type}\n\n\
        对应字幕片段:\n{subtitle_snippet}\n\n\
        请判断类型、信息增量、相似度，输出一行 JSON。"
    )
}

fn build_batch_review_prompt(frames: &[serde_json::Value], subtitle_srt: &str) -> String {
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

    format!(
        "请审查以下 {} 张视频截图（图片按顺序附在下方）。\n\n\
        ## 截图列表\n{}\n\n\
        ## 字幕参考\n{subtitle_summary}\n\n\
        为每张截图输出一行 JSON 审查结果，逐行输出，不要用数组包裹。",
        frames.len(),
        frame_list.join("\n"),
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
