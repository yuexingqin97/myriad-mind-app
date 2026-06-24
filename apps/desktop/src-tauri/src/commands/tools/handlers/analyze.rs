// Analyze 阶段工具：关键帧抽取 + 截图审查（均为 Cost::Free）
// 复用 pipeline::extract_keyframes_guided 与 ai::vision::review_keyframes，逻辑不动。

use crate::commands::ai::types::ScreenshotReviewConfig;
use crate::commands::ai::vision::review_keyframes;
use crate::commands::pipeline::extract_keyframes_guided;
use crate::commands::tools::{
    ArtifactKind, ArtifactRef, Cost, Phase, ToolContext, ToolFuture, ToolHandler, ToolOutput,
    ToolSpec, require_str,
};
use crate::error::AppError;
use std::path::PathBuf;

/// 关键帧抽取：Rust 直调 FFmpeg（scene 模式 + 可选引导时间戳），
/// 把 .png 截图目录以 Screenshots artifact 引用回喂。
pub struct ExtractKeyframesHandler;

impl ToolHandler for ExtractKeyframesHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "extract_keyframes".into(),
            description: "从视频中抽取关键帧截图（场景切换 + 可选字幕引导时间戳）。返回截图目录 artifact。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "video_path": {
                        "type": "string",
                        "description": "本地视频文件绝对路径"
                    },
                    "timestamps_path": {
                        "type": "string",
                        "description": "可选：字幕分析生成的引导时间戳 JSON 文件路径"
                    }
                },
                "required": ["video_path"]
            }),
            phase: Phase::Analyze,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let video_path = require_str(&params, "video_path")?;
            let timestamps_path = crate::commands::tools::opt_str(&params, "timestamps_path");

            let video = PathBuf::from(&video_path);
            // 输出目录：temp/keyframes（每次任务一份）
            let output_dir = ctx.temp_dir.join("keyframes");

            // 2. 调底层抽取函数（同步 block_in_place，内部已封装）
            //    guided_timestamps 传文件不存在时底层自动忽略。
            let guided = timestamps_path.as_ref().map(|s| std::path::Path::new(s));
            extract_keyframes_guided(
                &video,
                &output_dir,
                guided,
            )?;

            // 3. media::extract_keyframes_impl 把 PNG + keyframes.json 落在 output_dir/frames/ 子目录
            //    （frames_dir = output_dir/"frames"）。统计该子目录下 .png 数量。
            let frames_dir = output_dir.join("frames");
            let mut frame_count: u64 = 0;
            if let Ok(entries) = std::fs::read_dir(&frames_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().and_then(|e| e.to_str()) == Some("png") {
                        frame_count += 1;
                    }
                }
            }

            // 4. artifact 指向 frames 子目录（含 png + keyframes.json，供 review_keyframes 用）
            let tokens_estimate = frame_count.saturating_mul(500);
            let art = ArtifactRef {
                id: "keyframes/".into(),
                path: frames_dir.clone(),
                kind: ArtifactKind::Screenshots,
                tokens_estimate,
                summary: format!("{frame_count} 帧截图"),
            };

            Ok(ToolOutput::artifact(
                format!("已抽取 {frame_count} 帧关键帧，存于 {}", frames_dir.display()),
                art,
            ))
        })
    }
}

/// 截图审查：调 DeepSeek Vision 审查候选截图，回喂审查表摘要。
/// 需要 frames_dir 下存在 keyframes.json（抽取脚本产物）；transcript 可选。
pub struct ReviewKeyframesHandler;

impl ToolHandler for ReviewKeyframesHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "review_keyframes".into(),
            description: "用视觉模型审查候选关键帧，按信息增量打分筛选，返回审查表与选中截图清单。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "frames_dir": {
                        "type": "string",
                        "description": "关键帧截图目录绝对路径（需含 keyframes.json）"
                    },
                    "transcript_path": {
                        "type": "string",
                        "description": "可选：字幕 SRT 文件路径，用于提供审查上下文"
                    }
                },
                "required": ["frames_dir"]
            }),
            phase: Phase::Analyze,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let frames_dir_str = require_str(&params, "frames_dir")?;
            let transcript_path = crate::commands::tools::opt_str(&params, "transcript_path");
            let frames_dir = std::path::Path::new(&frames_dir_str);

            // 2. 读取 keyframes.json（抽取脚本产出的帧清单）
            let keyframes_json_path = frames_dir.join("keyframes.json");
            let keyframes_text = std::fs::read_to_string(&keyframes_json_path).map_err(|e| {
                AppError::Other(format!(
                    "读取 keyframes.json 失败（{}）: {e}",
                    keyframes_json_path.display()
                ))
            })?;
            let keyframes_json: serde_json::Value = serde_json::from_str(&keyframes_text)
                .map_err(|e| AppError::Other(format!("keyframes.json 解析失败: {e}")))?;

            // 3. 读取字幕（可选）
            let subtitle_srt = match transcript_path.as_ref() {
                Some(p) => std::fs::read_to_string(p).unwrap_or_default(),
                None => String::new(),
            };

            // 4. 视觉模型预热：提前校验 API key，避免进 review 后才报错
            let _api_key = crate::commands::ai::engine::read_deepseek_key()?;

            // 5. 调视觉审查（默认配置：hybrid 模式）
            let _ = &ctx.app; // 预留：当前 review_keyframes 不直接用 app_handle
            let config = ScreenshotReviewConfig::default();
            let result = review_keyframes(
                &keyframes_json,
                &subtitle_srt,
                frames_dir,
                &config,
            )
            .await?;

            // 6. 构造输出：审查表回喂，结构化数据放 metadata
            let summary = if result.review_table.is_empty() {
                format!(
                    "截图审查完成：共 {} 张候选，选中 {} 张，跳过 {} 张",
                    result.total,
                    result.selected.len(),
                    result.skipped
                )
            } else {
                // review_table 已是 Markdown，作为摘要正文回喂给 LLM
                result.review_table.clone()
            };

            let selected_files: Vec<String> = result
                .selected
                .iter()
                .map(|f| format!("{} @{}", f.file, f.timestamp_label))
                .collect();

            let metadata = serde_json::json!({
                "total": result.total,
                "selected_count": result.selected.len(),
                "skipped": result.skipped,
                "selected_frames": selected_files,
                "frames_dir": frames_dir_str,
            });

            Ok(ToolOutput {
                summary,
                artifact_refs: vec![],
                metadata,
            })
        })
    }
}
