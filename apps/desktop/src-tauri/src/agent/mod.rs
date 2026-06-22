// ============================================================
// agent — 目标驱动的笔记炼化 agent（设计文档 §六）
//
// 六阶段骨架内，AI 自主选工具：Recall→Acquire→Analyze→Generate→Verify→Memorize。
// loop 用 DeepSeek tool use（非流式，OpenAI 兼容），工具结果 artifact 化回喂。
// 入口 runner::run(app, AgentRequest) → AgentResult，由 execute_pipeline 调度（Phase 3 接线）。
// ============================================================

pub mod charter;
pub mod context;
pub mod runner;

pub use runner::{run, AgentRequest, AgentResult};
