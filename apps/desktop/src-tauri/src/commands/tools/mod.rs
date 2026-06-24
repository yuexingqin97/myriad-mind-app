// ============================================================
// tools — Agent 工具层（设计文档 §五）
//
// 把现有 commands 的能力封装成白名单工具：ToolSpec（给 LLM 看的描述）+
// ToolHandler（统一执行接口）+ ToolOutput（artifact 优先，大结果落盘只回摘要）。
// 底层逻辑（Python 脚本 / FFmpeg / fetch / fs）一律复用，不动。
//
// 动态分发：trait 方法手动 desugar 为 Pin<Box<dyn Future>>，支持 Box<dyn ToolHandler>
// 注册表按名字调度，无需引入 async-trait 依赖。
// ============================================================

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tauri::AppHandle;

pub mod handlers;
pub mod registry;

// ============================================================
// 执行上下文
// ============================================================

/// 工具执行上下文：每个 agent 任务构造一份，贯穿该任务所有工具调用。
///
/// Phase 1 不含取消令牌；Phase 2 agent loop 注入 CancellationToken（设计 §6.5），
/// 届时在此结构追加字段并在长工具内 `select!` 检查。
pub struct ToolContext {
    /// Tauri 应用句柄（用于 emit pipeline-progress / mind-stream 事件）
    pub app: AppHandle,
    /// 解析到的 Python 路径（用户配置 > venv > PATH）
    pub python_path: String,
    /// 本次任务的临时工作目录（下载 / 中间产物）
    pub temp_dir: PathBuf,
    /// artifact 落盘根目录（字幕/正文/截图/草稿等大结果存这里，ToolOutput 只回引用）
    pub artifacts_dir: PathBuf,
    /// 笔记输出根目录（write_note / memory.md / 知识库索引）
    pub note_dir: String,
    /// 用户输入对应的本地路径（local_text/local_video/code_project 等模式），
    /// 作为 read 类工具的可信读取根之一；URL 输入为 None。
    pub input_root: Option<PathBuf>,
}

impl ToolContext {
    /// 计算/确保 artifacts_dir 存在（agent 启动时调用）
    pub fn ensure_artifacts_dir(&self) -> Result<(), AppError> {
        std::fs::create_dir_all(&self.artifacts_dir).map_err(AppError::Io)
    }

    /// 可信读取根：temp_dir / artifacts_dir / note_dir / input_root。
    /// read_file / scan_directory / scan_code_project / read_artifact 的路径必须落在其中之一，
    /// 防止 prompt injection 诱导 agent 读取 ~/.ssh/id_rsa、.env 等敏感文件（对抗审查 critical）。
    fn read_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![
            self.temp_dir.clone(),
            self.artifacts_dir.clone(),
            PathBuf::from(&self.note_dir),
        ];
        if let Some(r) = &self.input_root {
            roots.push(r.clone());
        }
        roots
    }

    /// 把 requested 解析到 base 子树内（写沙箱）。绝对路径直接用，相对路径基于 base。
    /// canonicalize 后校验 starts_with(base)；目标尚不存在（写场景）时用 parent canonicalize + file_name。
    /// 越界或非法 → Err。write_note 用此方法锁定写入 note_dir。
    pub fn resolve_within(&self, base: &Path, requested: &str) -> Result<PathBuf, AppError> {
        let base_c = base.canonicalize().map_err(AppError::Io)?;
        let p = if Path::new(requested).is_absolute() {
            PathBuf::from(requested)
        } else {
            base.join(requested)
        };
        let resolved = match p.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                // 目标文件可能尚不存在（write_note），用 parent canonicalize + file_name 兜底
                let parent = p.parent().unwrap_or(Path::new("."));
                let parent_c = parent.canonicalize().map_err(AppError::Io)?;
                let fname = p
                    .file_name()
                    .ok_or_else(|| AppError::Other("无效文件名".into()))?;
                parent_c.join(fname)
            }
        };
        if !resolved.starts_with(&base_c) {
            return Err(AppError::Other(format!(
                "路径越界，拒绝写入 note_dir 之外: {requested}"
            )));
        }
        Ok(resolved)
    }

    /// 可读路径解析：允许落在任一 read_roots 子树。越界 → Err。
    pub fn resolve_readable(&self, requested: &str) -> Result<PathBuf, AppError> {
        for base in self.read_roots() {
            // base 可能尚不存在（如 input_root 被删），canonicalize 失败则跳过该根
            let Ok(base_c) = base.canonicalize() else {
                continue;
            };
            let p = if Path::new(requested).is_absolute() {
                PathBuf::from(requested)
            } else {
                base.join(requested)
            };
            if let Ok(resolved) = p.canonicalize() {
                if resolved.starts_with(&base_c) {
                    return Ok(resolved);
                }
            }
        }
        let allowed: Vec<String> = self
            .read_roots()
            .iter()
            .filter_map(|p| p.canonicalize().ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        Err(AppError::Other(format!(
            "路径不在允许的读取范围内: {requested}\n\
             允许的根目录为：\n{}",
            allowed
                .iter()
                .map(|p| format!("  - {p}"))
                .collect::<Vec<_>>()
                .join("\n")
        )))
    }
}

// ============================================================
// 工具元数据
// ============================================================

/// agent 六阶段（设计文档 §四）。ToolSpec 标注工具的「主阶段」，供 charter 提示与诊断。
/// 注意：实际向 LLM 暴露的是全量（扣除花费）工具集 + 阶段指引，而非硬性按阶段切断——
/// 避免阶段边界卡死 agent。phase 字段作为文档/排序依据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Recall,
    Acquire,
    Analyze,
    Generate,
    Verify,
    Memorize,
}

/// 花费分级：Paid 工具在「省流」模式（config 开关）下从白名单剔除。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cost {
    Free,
    /// 消耗外部 API 额度（如 AI Douyin 下载）
    Paid,
}

/// 给 LLM 看的工具描述（对应 OpenAI tool use 的 function 定义）。
/// agent loop 据此 + input_schema 让 LLM 决定调不调、怎么传参。
#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema，约束 LLM 生成的参数
    pub input_schema: serde_json::Value,
    pub phase: Phase,
    pub cost: Cost,
}

// ============================================================
// artifact（设计文档 §6.6.2）
// ============================================================

/// artifact 类型。大结果落盘后用 ArtifactRef 引用，不在 messages 里放全文。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Transcript,
    ArticleText,
    CodeScan,
    Screenshots,
    Subtitle,
    Draft,
    VideoFile,
    AudioFile,
}

impl ArtifactKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Transcript => "transcript",
            Self::ArticleText => "article_text",
            Self::CodeScan => "code_scan",
            Self::Screenshots => "screenshots",
            Self::Subtitle => "subtitle",
            Self::Draft => "draft",
            Self::VideoFile => "video_file",
            Self::AudioFile => "audio_file",
        }
    }
}

/// artifact 引用：大结果落盘后的指针。
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactRef {
    /// 稳定 id，如 "transcript.vtt"、"article.md"、"keyframes/"
    pub id: String,
    /// 落盘路径
    pub path: PathBuf,
    pub kind: ArtifactKind,
    /// 粗估 token 数（供 context manager 预算）
    pub tokens_estimate: u64,
    /// 单行描述
    pub summary: String,
}

impl ArtifactRef {
    /// 粗估 token：中文 ~2 字符/token，英文 ~4 字符/token，取保守的 chars/3。
    pub fn estimate_tokens(text: &str) -> u64 {
        (text.chars().count() as u64 / 3).max(1)
    }
}

// ============================================================
// 工具输出
// ============================================================

/// 工具统一返回：给 LLM 的短摘要 + artifact 引用 + 元信息。
/// 大文本（字幕/正文/扫描结果/截图）绝不进 summary，只给 artifact 引用。
#[derive(Debug, Clone, Serialize)]
pub struct ToolOutput {
    pub summary: String,
    pub artifact_refs: Vec<ArtifactRef>,
    pub metadata: serde_json::Value,
}

impl ToolOutput {
    /// 只有摘要、无 artifact（小结果，如 read_file 小文件、query 结果）。
    pub fn text(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            artifact_refs: vec![],
            metadata: serde_json::json!({}),
        }
    }

    /// 单 artifact + 摘要。
    pub fn artifact(summary: impl Into<String>, art: ArtifactRef) -> Self {
        Self {
            summary: summary.into(),
            artifact_refs: vec![art],
            metadata: serde_json::json!({}),
        }
    }

    /// 序列化为回喂 LLM 的 tool_result 文本（OpenAI tool message content）。
    /// 只含 summary + artifact 引用清单，不含全文。
    pub fn to_llm_text(&self) -> String {
        let mut s = self.summary.clone();
        if !self.artifact_refs.is_empty() {
            s.push_str("\n\n[artifacts]");
            for a in &self.artifact_refs {
                s.push_str(&format!(
                    "\n- {} ({}, ~{} tok): {}",
                    a.id,
                    a.kind.as_str(),
                    a.tokens_estimate,
                    a.summary
                ));
            }
        }
        s
    }
}

// ============================================================
// 工具执行接口
// ============================================================

/// 工具 future：手动 desugar 的 async trait 返回类型（dyn-compatible，无需 async-trait 依赖）。
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<ToolOutput, AppError>> + Send + 'a>>;

/// 工具执行接口。每个工具一个实现，注册到 ToolRegistry。
pub trait ToolHandler: Send + Sync {
    /// 工具描述（给 LLM 看）。
    fn spec(&self) -> ToolSpec;
    /// 执行。params 是 LLM 按 input_schema 生成的 JSON。
    fn handle<'a>(&'a self, ctx: &'a ToolContext, params: serde_json::Value) -> ToolFuture<'a>;
}

/// 从 params 取必需字段，缺失返回清晰的 AppError。
pub fn require_str(params: &serde_json::Value, key: &str) -> Result<String, AppError> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Other(format!("工具参数缺失或非字符串: {key}")))
}

/// 从 params 取可选字符串字段。
pub fn opt_str(params: &serde_json::Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}
