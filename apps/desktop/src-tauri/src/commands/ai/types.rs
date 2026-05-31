// ============================================================
// AI 模块类型定义
// ============================================================

use serde::{Deserialize, Serialize};

// ---- 任务类型 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiTask {
    #[serde(rename = "note_generation")]
    NoteGeneration,
    #[serde(rename = "summary")]
    Summary,
    #[serde(rename = "translation")]
    Translation,
    #[serde(rename = "code_analysis")]
    CodeAnalysis,
    #[serde(rename = "compare")]
    Compare,
    #[serde(rename = "resource_recommend")]
    ResourceRecommend,
    #[serde(rename = "next_step_suggestion")]
    NextStepSuggestion,
    #[serde(rename = "screenshot_review")]
    ScreenshotReview,
    #[serde(rename = "subtitle_analysis")]
    SubtitleAnalysis,
    #[serde(rename = "tutorial_detection")]
    TutorialDetection,
}

impl std::fmt::Display for AiTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoteGeneration => write!(f, "生成学习笔记"),
            Self::Summary => write!(f, "摘要"),
            Self::Translation => write!(f, "翻译"),
            Self::CodeAnalysis => write!(f, "代码分析"),
            Self::Compare => write!(f, "对比"),
            Self::ResourceRecommend => write!(f, "资源推荐"),
            Self::NextStepSuggestion => write!(f, "下一步建议"),
            Self::ScreenshotReview => write!(f, "截图审查"),
            Self::SubtitleAnalysis => write!(f, "字幕分析"),
            Self::TutorialDetection => write!(f, "教程检测"),
        }
    }
}

// ---- 思考配置 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReasoningEffort {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "max")]
    Max,
}

// ---- 消息 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}

// ---- 请求 / 响应 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindRequest {
    pub task: AiTask,
    pub messages: Vec<AiMessage>,
    pub system_prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_override: Option<String>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MindResponse {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_text: Option<String>,
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

// ---- 流式事件 (统一事件格式，推到前端) ----

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum MindStreamEvent {
    #[serde(rename = "start")]
    Start {
        task: String,
        provider: String,
        model: String,
    },
    #[serde(rename = "reasoning_delta")]
    ReasoningDelta { delta: String },
    #[serde(rename = "delta")]
    Delta { delta: String },
    #[serde(rename = "usage")]
    Usage {
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        reasoning_tokens: Option<u32>,
        total_tokens: Option<u32>,
    },
    #[serde(rename = "done")]
    Done {
        text: String,
        finish_reason: Option<String>,
    },
    #[allow(dead_code)]
    #[serde(rename = "error")]
    Error {
        code: String,
        message: String,
        retryable: bool,
    },
}

// ---- 错误分类 ----

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub enum AiErrorKind {
    Authentication,
    RateLimited,
    Network,
    Timeout,
    Server,
    ContextLength,
    ContentPolicy,
    ModelNotFound,
    ProviderNotConfigured,
    UnsupportedFeature,
    InvalidResponse,
}

impl AiErrorKind {
    pub fn classify(status: Option<u16>, body: &str) -> &'static str {
        match status {
            Some(401) => "authentication",
            Some(429) => "rate_limited",
            Some(402) => "rate_limited",
            Some(404) if body.contains("model") => "model_not_found",
            Some(404) => "provider_not_configured",
            Some(413) => "context_length",
            Some(500..=599) => "server_error",
            None => "network",
            _ => "invalid_response",
        }
    }

    #[allow(dead_code)]
    pub fn user_message(kind: &str) -> &str {
        match kind {
            "authentication" => "DeepSeek API Key 无效，请重新配置",
            "rate_limited" => "请求过于频繁或余额不足，稍后重试",
            "network" => "网络连接失败，请检查代理或网络",
            "timeout" => "请求超时，请重试",
            "server_error" => "DeepSeek 服务临时错误，稍后重试",
            "context_length" => "内容超过上下文限制，请分段处理",
            "model_not_found" => "模型不存在，请检查模型名称",
            "provider_not_configured" => "未配置 DeepSeek API Key",
            _ => "AI 响应解析失败",
        }
    }
}

// ============================================================
// 视觉功能类型 (DeepSeek V4 Vision)
// ============================================================

/// 视觉消息（支持图片 + 文本）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionMessage {
    pub role: String,
    /// content 为 [{type: "text", text: "..."}, {type: "image_url", image_url: {url: "..."}}]
    pub content: Vec<VisionContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VisionContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// base64 data URL (data:image/png;base64,...) or http URL
    pub url: String,
    /// 可选 detail 字段（DeepSeek 兼容但不强制）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// 视觉请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionRequest {
    pub task: AiTask,
    pub messages: Vec<VisionMessage>,
    pub system_prompt: String,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_override: Option<String>,
}

// ---- 字幕分析 ----

/// 字幕引导截图时间点（步骤 4.5 输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidedTimestamp {
    pub ts: f64,
    pub reason: String,
}

// ---- 截图审查 ----

/// 单张截图的审查结论（步骤 7.1 输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameReview {
    pub type_tag: String,
    /// 信息增量评分 0-3
    pub info_score: u8,
    /// 与上一张的相似度 0.0-1.0
    pub similarity_vs_prev: f64,
    /// 推荐嵌入的笔记章节
    pub embed_section: String,
    /// 审查理由（20 字以内）
    pub reason: String,
}

/// 审查通过的截图
#[derive(Debug, Clone, Serialize)]
pub struct ReviewedFrame {
    pub file: String,
    pub timestamp_seconds: f64,
    pub timestamp_label: String,
    pub trigger: String,
    pub review: FrameReview,
}

/// 截图审查汇总
#[derive(Debug, Clone, Serialize)]
pub struct ScreenshotReviewResult {
    pub total: usize,
    pub selected: Vec<ReviewedFrame>,
    pub skipped: usize,
    /// 审查表（Markdown 格式，供注入笔记 prompt）
    pub review_table: String,
}

// ---- 教程检测 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialDetectionResult {
    pub is_tutorial: bool,
    pub confidence: f64,
    pub signals: Vec<String>,
}

// ---- 截图审查配置 ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotReviewConfig {
    pub enabled: bool,
    pub mode: String, // "batch" | "single" | "hybrid"
    pub max_review_frames: usize,
    pub min_score: u8,
    pub max_selected: usize,
}

impl Default for ScreenshotReviewConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: "hybrid".into(),
            max_review_frames: 25,
            min_score: 2,
            max_selected: 15,
        }
    }
}
