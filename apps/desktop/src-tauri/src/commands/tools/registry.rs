// ============================================================
// ToolRegistry — 按名字注册/调度工具，按花费开关过滤白名单。
// agent loop（Phase 2）通过 all_specs() 向 LLM 暴露工具，通过 dispatch() 执行。
// ============================================================

use super::handlers;
use super::{Cost, ToolContext, ToolHandler, ToolOutput, ToolSpec};
use crate::error::AppError;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ToolRegistry {
    by_name: HashMap<String, Arc<dyn ToolHandler>>,
    order: Vec<String>,
}

impl ToolRegistry {
    /// 注册全部内置工具。
    pub fn build() -> Self {
        let all: Vec<Arc<dyn ToolHandler>> = vec![
            // acquire（获取阶段）
            Arc::new(handlers::FetchUrlHandler),
            Arc::new(handlers::DownloadVideoHandler),
            Arc::new(handlers::ExtractAudioHandler),
            Arc::new(handlers::TranscribeAsrHandler),
            Arc::new(handlers::DownloadSubtitlesHandler),
            Arc::new(handlers::ScanCodeProjectHandler),
            Arc::new(handlers::ReadFileHandler),
            Arc::new(handlers::ScanDirectoryHandler),
            Arc::new(handlers::QueryAiDouyinHandler),
            // analyze（理解阶段）
            Arc::new(handlers::ExtractKeyframesHandler),
            Arc::new(handlers::ReviewKeyframesHandler),
            // io（生成/自检阶段）
            Arc::new(handlers::WriteNoteHandler),
            Arc::new(handlers::ReadArtifactHandler),
        ];
        let mut by_name = HashMap::new();
        let mut order = Vec::new();
        for h in all {
            let name = h.spec().name;
            order.push(name.clone());
            by_name.insert(name, h);
        }
        Self { by_name, order }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.by_name.get(name).cloned()
    }

    pub fn has(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    /// 全量工具 spec（按花费开关过滤 Paid），向 LLM 暴露。
    pub fn all_specs(&self, allow_paid: bool) -> Vec<ToolSpec> {
        self.order
            .iter()
            .filter_map(|n| self.by_name.get(n))
            .map(|h| h.spec())
            .filter(|s| allow_paid || s.cost != Cost::Paid)
            .collect()
    }

    /// 按 name 调度执行。未知工具返回 AppError。
    pub async fn dispatch(
        &self,
        name: &str,
        ctx: &ToolContext,
        params: serde_json::Value,
    ) -> Result<ToolOutput, AppError> {
        match self.by_name.get(name) {
            Some(h) => h.handle(ctx, params).await,
            None => Err(AppError::Other(format!("未知工具: {name}"))),
        }
    }
}
