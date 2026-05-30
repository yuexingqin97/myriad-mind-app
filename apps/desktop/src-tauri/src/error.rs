// ============================================================
// 统一错误类型 — 所有 Tauri 命令返回此类型
// ============================================================

use serde::Serialize;

/// App 全局错误类型
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Python 脚本执行失败: {script} — {stderr}")]
    PythonScript { script: String, stderr: String },

    #[error("依赖缺失: {0}")]
    MissingDependency(String),

    #[error("依赖版本不满足: {dep} 需要 >= {required}, 当前 {actual}")]
    DepVersion {
        dep: String,
        required: String,
        actual: String,
    },

    #[error("配置错误: {0}")]
    Config(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP 错误: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Claude API 错误: {code} — {message}")]
    ClaudeApi { code: u16, message: String },

    #[error("流式响应中断: {0}")]
    StreamError(String),

    #[error("已取消")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
