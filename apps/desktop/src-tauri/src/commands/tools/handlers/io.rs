// ============================================================
// Generate 阶段工具 — 笔记写入 / artifact 召回
//
// write_note：把 agent 生成的 Markdown 笔记落盘到 note_dir。
// read_artifact：agent 召回先前步骤落盘的 artifact 全文（字幕/正文/扫描结果等），
//                超过 max_chars 则截断，避免一次性灌满 context。
// ============================================================

use crate::commands::fs::write_note;
use crate::commands::tools::{
    ArtifactKind, ArtifactRef, Cost, Phase, ToolContext, ToolFuture, ToolHandler, ToolOutput,
    ToolSpec, require_str,
};
use crate::error::AppError;
use std::path::PathBuf;

// ------------------------------------------------------------
// write_note — 写 Markdown 笔记
// ------------------------------------------------------------

/// 写入 Markdown 笔记到指定路径（通常位于 note_dir 下）。
pub struct WriteNoteHandler;

impl ToolHandler for WriteNoteHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_note".into(),
            description: "将 Markdown 笔记写入指定文件路径（含父目录自动创建）。用于落盘 agent 生成的最终笔记。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "目标文件绝对或相对路径（相对路径基于 note_dir），例如 \"out/线性代数.md\""
                    },
                    "content": {
                        "type": "string",
                        "description": "Markdown 全文内容"
                    }
                },
                "required": ["path", "content"]
            }),
            phase: Phase::Generate,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数
            let path = require_str(&params, "path")?;
            let content = require_str(&params, "content")?;

            // 2. 沙箱：锁定写入 note_dir 子树（绝对/相对路径都解析到 note_dir 内，防越权写）
            let resolved: PathBuf = ctx.resolve_within(std::path::Path::new(&ctx.note_dir), &path)?;
            write_note(resolved.to_string_lossy().to_string(), content.clone()).await?;

            // 3. 构造 Draft artifact 引用
            let id = resolved
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| resolved.to_string_lossy().to_string());
            let char_count = content.chars().count();
            let artifact = ArtifactRef {
                id,
                path: resolved.clone(),
                kind: ArtifactKind::Draft,
                tokens_estimate: ArtifactRef::estimate_tokens(&content),
                summary: format!("已写入笔记 {char_count} 字"),
            };

            Ok(ToolOutput::artifact(
                format!("笔记已写入：{}（{char_count} 字）", resolved.display()),
                artifact,
            ))
        })
    }
}

// ------------------------------------------------------------
// read_artifact — 召回 artifact 全文
// ------------------------------------------------------------

/// 读取 artifact 落盘全文。内容 <= max_chars 直接返回；超出则截断前 max_chars 字符并标注。
pub struct ReadArtifactHandler;

impl ToolHandler for ReadArtifactHandler {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_artifact".into(),
            description: "读取先前步骤落盘的 artifact 全文（字幕、网页正文、代码扫描、草稿等）。内容过长将截断到 max_chars 字符并附总数提示。".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "artifact 文件路径（来自先前工具返回的 artifact_ref.path）"
                    },
                    "max_chars": {
                        "type": "number",
                        "description": "最多返回的字符数，默认 20000。超出则截断前 max_chars 字符。",
                        "default": 20000
                    }
                },
                "required": ["path"]
            }),
            phase: Phase::Generate,
            cost: Cost::Free,
        }
    }

    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a> {
        Box::pin(async move {
            // 1. 解析参数：path 必填，max_chars 可选（默认 20000）
            let path = require_str(&params, "path")?;
            let max_chars = params
                .get("max_chars")
                .and_then(|v| v.as_u64())
                .unwrap_or(20_000) as usize;

            // 2. 沙箱：限制在可信读取根内（artifact 应来自 temp/artifacts/note_dir）
            let resolved = ctx.resolve_readable(&path)?;
            let content = std::fs::read_to_string(&resolved).map_err(AppError::Io)?;
            let total = content.chars().count();

            // 3. 截断判断：<= max_chars 直接回，超过取前 max_chars 字符 + 标注
            let output = if total <= max_chars {
                content
            } else {
                let head: String = content.chars().take(max_chars).collect();
                format!("{head}\n\n(已截断，共 {total} 字符)")
            };

            Ok(ToolOutput::text(output))
        })
    }
}
