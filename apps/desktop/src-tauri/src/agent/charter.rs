// ============================================================
// charter — 组装 agent system prompt（设计文档 §6.2）
//
// 模板在 packages/core/prompts/agent/charter.md，由 PromptManager 渲染。
// 工具说明从 ToolSpec 自动生成（不手写）；task_state/memory/task_prompt 注入。
// ============================================================

use super::context::TaskState;
use crate::commands::ai::prompt_manager::PromptManager;
use crate::commands::tools::registry::ToolRegistry;
use crate::error::AppError;

/// 构建 agent system prompt。
///
/// `allow_paid` 控制是否向 agent 暴露花费工具（设计 §6.3 花费开关）。
pub fn build(
    registry: &ToolRegistry,
    allow_paid: bool,
    task_state: &TaskState,
    memory: &str,
    task_prompt: Option<&str>,
) -> Result<String, AppError> {
    let pm = PromptManager::new()?;

    // 工具清单：从 ToolSpec 自动生成（name + description），不手写。
    let tools_description: String = registry
        .all_specs(allow_paid)
        .iter()
        .map(|s| format!("- **{}**：{}", s.name, s.description))
        .collect::<Vec<_>>()
        .join("\n");

    let rendered = pm.render(
        "agent/charter",
        minijinja::context! {
            tools_description => tools_description,
            task_state_yaml => task_state.to_yaml(),
            memory => memory,
            task_prompt => task_prompt,
        },
    )?;
    Ok(rendered)
}
