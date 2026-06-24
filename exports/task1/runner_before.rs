// ============================================================
// runner — agent 主循环（设计文档 §6.1 loop / §6.3 护栏 / §6.4 错误 / §6.5 取消）
//
// 流程：构造 ToolContext + Registry → 回忆(加载 memory) → charter system prompt →
//   loop { chat_turn(tools) → 有 tool_calls 则 dispatch + 回喂；无则终结产出笔记 }
// 护栏：MAX_STEPS 强制结束。取消：AtomicBool 每轮检查（Phase 3 由 Tauri 命令置位）。
//
// 非流式：tool-use 轮用 chat_turn（非流式，OpenAI 兼容），最终笔记经 mind-stream 发出。
// （流式 + tool_call 分片累积未在本项目验证，v1 用非流式更可靠；见计划文档 Spike 说明。）
// ============================================================

use super::charter;
use super::context::TaskState;
use crate::commands::ai::deepseek::chat_turn;
use crate::commands::ai::engine::read_deepseek_key;
use crate::commands::ai::types::{MindStreamEvent, ReasoningEffort, ThinkingConfig};
use crate::commands::pipeline::{emit_progress, generate_temp_id};
use crate::commands::tools::registry::ToolRegistry;
use crate::commands::tools::{ArtifactKind, ToolContext, ToolSpec};
use crate::error::AppError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// agent 单任务最大轮数（护栏，防失控死循环）。
const MAX_STEPS: u32 = 25;

/// agent 运行请求。
pub struct AgentRequest {
    /// 用户原始输入（URL / 路径 / 文本）
    pub input: String,
    /// 笔记输出根目录
    pub note_dir: String,
    /// 解析到的 Python 路径
    pub python_path: String,
    /// 用户「本次要求」（prompt hook）
    pub task_prompt: Option<String>,
    /// 花费开关：false 则隐藏 Paid 工具（如 query_ai_douyin）
    pub allow_paid: bool,
}

/// agent 运行结果。
pub struct AgentResult {
    /// 最终笔记 Markdown
    pub note_content: String,
    /// 笔记落盘路径（Phase 3 持久化时填；Phase 2 为 None）
    pub note_path: Option<String>,
    /// 总轮数
    pub steps: u32,
    /// 调用过的工具名（去重）
    pub tools_used: Vec<String>,
    /// 累计 token（所有轮 usage 之和）
    pub total_tokens: u64,
}

/// 主入口：跑完整 agent 六阶段，产出笔记 Markdown。
///
/// 通过 `pipeline-progress` 推送阶段进度，通过 `mind-stream` 推送最终笔记。
/// 由 execute_pipeline（Phase 3 接线）调度。
pub async fn run(app: &AppHandle, req: AgentRequest) -> Result<AgentResult, AppError> {
    let api_key = read_deepseek_key()?;
    let registry = ToolRegistry::build();

    // 任务临时目录 + artifact 目录（字幕/正文/截图落盘于此，ToolOutput 只回引用）
    let task_id = generate_temp_id(&req.input);
    let temp_dir = std::env::temp_dir().join("myriad-mind").join(&task_id);
    let artifacts_dir = temp_dir.join("artifacts");
    std::fs::create_dir_all(&temp_dir).map_err(AppError::Io)?;
    std::fs::create_dir_all(&artifacts_dir).map_err(AppError::Io)?;

    // input_root：本地输入（文件/目录）作为 read 类工具的可信读取根；URL 输入为 None
    let input_root = {
        let p = std::path::Path::new(&req.input);
        if p.exists() {
            Some(p.to_path_buf())
        } else {
            None
        }
    };
    let ctx = ToolContext {
        app: app.clone(),
        python_path: req.python_path.clone(),
        temp_dir: temp_dir.clone(),
        artifacts_dir: artifacts_dir.clone(),
        note_dir: req.note_dir.clone(),
        input_root,
    };

    // 阶段 0 回忆：加载知识库记忆 + 知识库索引摘要
    emit_progress(app, "recall", "🧠 回忆：加载知识库记忆", 2.0, "running", None);
    let memory = load_memory(&req.note_dir);
    let mut task_state = TaskState::new(&req.input);
    emit_progress(app, "recall", "回忆完成", 5.0, "completed", None);

    // 向 LLM 暴露的工具（OpenAI function 格式）
    let tools_for_llm: Vec<serde_json::Value> = registry
        .all_specs(req.allow_paid)
        .iter()
        .map(spec_to_openai)
        .collect();

    // agent 工作记忆（OpenAI messages 格式）。首条 user 消息给出任务。
    let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({
        "role": "user",
        "content": format!(
            "请把以下输入炼化成结构化学习笔记。\n\n【输入】\n{input}\n\n按 charter 的六阶段骨架与输出契约执行：\
             先用工具获取/理解内容，素材齐备后，把完整笔记 Markdown 作为最终回复内容返回（不要再调工具）。",
            input = req.input
        ),
    })];

    let thinking = ThinkingConfig {
        enabled: true,
        effort: Some(ReasoningEffort::High),
    };
    // 取消标志（Phase 3 由 Tauri 命令置位；Phase 2 始终 false，保留检查点）。
    let cancel: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // 通知前端 AI 开始
    let _ = app.emit(
        "mind-stream",
        MindStreamEvent::Start {
            task: "note_generation".into(),
            provider: "deepseek".into(),
            model: "deepseek-v4-pro".into(),
        },
    );

    let mut steps = 0u32;
    let mut tools_used: Vec<String> = Vec::new();
    let mut total_tokens = 0u64;
    let mut final_content = String::new();

    while steps < MAX_STEPS {
        // 取消检查点（每轮顶部）
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        steps += 1;
        emit_progress(
            app,
            "agent",
            &format!("🤖 agent 推理（第 {steps}/{MAX_STEPS} 轮）"),
            10.0 + (steps as f64 / MAX_STEPS as f64) * 70.0,
            "running",
            None,
        );

        // 重新渲染 charter（task_state 每轮更新 → 工作面板最新）
        let system_prompt = charter::build(
            &registry,
            req.allow_paid,
            &task_state,
            &memory,
            req.task_prompt.as_deref(),
        )?;

        let mut turn =
            chat_turn(app, &system_prompt, &messages, &tools_for_llm, Some(&thinking), &api_key)
                .await?;

        if let Some(u) = &turn.usage {
            total_tokens += u.total_tokens as u64;
        }

        // 部分 OpenAI 兼容端点要求 assistant.content 为字符串（非 null），归一化避免拒收
        if turn
            .message
            .get("content")
            .map(|c| c.is_null())
            .unwrap_or(false)
        {
            turn.message["content"] = serde_json::json!("");
        }

        if turn.tool_calls.is_empty() {
            // 终止轮：最终笔记内容
            final_content = turn.content.clone().unwrap_or_default();
            messages.push(turn.message);
            log::debug!(
                target: "agent",
                "[agent] terminate step={steps} content_chars={} tools_used={:?}",
                final_content.chars().count(),
                tools_used
            );
            break;
        }

        // 有工具调用：先 append assistant message（含 tool_calls），再逐个执行 + append tool result
        messages.push(turn.message);
        for tc in &turn.tool_calls {
            if !tools_used.contains(&tc.function.name) {
                tools_used.push(tc.function.name.clone());
            }
            emit_progress(
                app,
                "agent",
                &format!("🔧 调用工具：{}", tc.function.name),
                10.0 + (steps as f64 / MAX_STEPS as f64) * 70.0,
                "running",
                None,
            );

            // 解析参数（模型给的 arguments 是 JSON 字符串）。失败则回喂诊断 + 跳过本次 dispatch，
            // 避免空对象 {} 命中 require_str 后无谓消耗一轮（对抗审查 finding）
            let params: serde_json::Value = match serde_json::from_str(&tc.function.arguments) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(
                        target: "agent",
                        "[agent] tool {} args parse failed: {e}; raw={}",
                        tc.function.name,
                        tc.function.arguments
                    );
                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": format!(
                            "⚠️ 工具 {} 的 arguments 不是合法 JSON（{}）。\n原始参数：{}\n请重新输出合法 JSON object。",
                            tc.function.name, e, tc.function.arguments
                        ),
                    }));
                    continue;
                }
            };

            // 执行工具（失败回喂错误让 agent 自修，见 §6.4）
            let result_text = match registry
                .dispatch(&tc.function.name, &ctx, params)
                .await
            {
                Ok(out) => {
                    // 记录产出的 artifact 到 task_state（下一轮 charter 可见）
                    for art in &out.artifact_refs {
                        task_state.add_artifact(art.clone());
                    }
                    if !out.artifact_refs.is_empty() {
                        let names: Vec<&str> =
                            out.artifact_refs.iter().map(|a| a.id.as_str()).collect();
                        task_state.decisions.push(format!(
                            "产出 artifact: {}",
                            names.join(", ")
                        ));
                    }
                    out.to_llm_text()
                }
                Err(e) => {
                    log::warn!(
                        target: "agent",
                        "[agent] tool {} failed: {e}",
                        tc.function.name
                    );
                    // 回喂错误。stderr 已在 python.rs 源头脱敏（redact_secrets），不含明文 api_key；
                    // 其他 AppError 变体（Config/Other）只含路径/消息，无密钥。
                    format!("⚠️ 工具 {} 执行失败：{}", tc.function.name, e)
                }
            };

            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": result_text,
            }));
        }
    }

    if final_content.trim().is_empty() {
        // 兜底：agent 可能用 write_note 落盘后未再发纯文本收尾轮（或触顶 MAX_STEPS），
        // 此时 final_content 为空但 Draft artifact 已在盘上——回收它作为最终笔记，避免白跑一轮报错。
        if let Some(draft) = task_state
            .artifact_refs
            .iter()
            .rev()
            .find(|a| a.kind == ArtifactKind::Draft)
        {
            match std::fs::read_to_string(&draft.path) {
                Ok(c) if !c.trim().is_empty() => {
                    log::warn!(
                        target: "agent",
                        "[agent] final_content 为空，回收 Draft artifact: {}",
                        draft.path.display()
                    );
                    final_content = c;
                }
                Ok(_) => {
                    return Err(AppError::Other(format!(
                        "agent 已写入 Draft 但内容为空: {}（path={}）",
                        draft.id,
                        draft.path.display()
                    )));
                }
                Err(e) => {
                    return Err(AppError::Other(format!(
                        "agent 标记已产出 Draft 但读取失败: {e}（path={}）",
                        draft.path.display()
                    )));
                }
            }
        } else {
            return Err(AppError::Other(format!(
                "agent 在 {MAX_STEPS} 步内未产出最终笔记，task_state.artifact_refs 中也无 Draft\
                 （已产出 {} 个 artifact，可能陷入工具循环，请检查日志）",
                task_state.artifact_refs.len()
            )));
        }
    }

    // 通过 mind-stream 分块发出最终笔记（匹配前端增量累积逻辑，每 ~1500 字一帧；
    // 按字符边界切片，避免拆分中文字符）
    {
        let chars: Vec<char> = final_content.chars().collect();
        const CHUNK: usize = 1500;
        let mut i = 0;
        while i < chars.len() {
            let end = (i + CHUNK).min(chars.len());
            let frame: String = chars[i..end].iter().collect();
            let _ = app.emit("mind-stream", MindStreamEvent::Delta { delta: frame });
            i = end;
        }
    }
    let _ = app.emit(
        "mind-stream",
        MindStreamEvent::Done {
            text: final_content.clone(),
            finish_reason: Some("stop".into()),
        },
    );

    // 不在此 emit "completed"——pipeline.rs 在 save/cleanup 之后才发真正的 step="completed"，
    // 这里提前发会让前端误判管线结束（对抗审查 frontend finding）。

    Ok(AgentResult {
        note_content: final_content,
        note_path: None,
        steps,
        tools_used,
        total_tokens,
    })
}

// ---- 辅助 ----

/// ToolSpec → OpenAI function tool 定义。
fn spec_to_openai(spec: &ToolSpec) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": spec.name,
            "description": spec.description,
            "parameters": spec.input_schema,
        }
    })
}

/// 读取目录级记忆 memory.md（设计 §四「上下文注入」）。
fn load_memory(note_dir: &str) -> String {
    let p = std::path::PathBuf::from(note_dir)
        .join(".myriad-mind")
        .join("memory.md");
    if p.exists() {
        std::fs::read_to_string(&p).unwrap_or_default()
    } else {
        String::new()
    }
}
