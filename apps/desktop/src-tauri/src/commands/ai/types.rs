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
    Start { task: String, provider: String, model: String },
    #[serde(rename = "reasoning_delta")]
    ReasoningDelta { delta: String },
    #[serde(rename = "delta")]
    Delta { delta: String },
    #[serde(rename = "usage")]
    Usage { input_tokens: Option<u32>, output_tokens: Option<u32>, reasoning_tokens: Option<u32>, total_tokens: Option<u32> },
    #[serde(rename = "done")]
    Done { text: String, finish_reason: Option<String> },
    #[serde(rename = "error")]
    Error { code: String, message: String, retryable: bool },
}

// ---- 错误分类 ----

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
