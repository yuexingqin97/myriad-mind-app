// ============================================================
// TaskState — agent 的结构化任务状态（设计文档 §6.6.3）
//
// 由框架在工具产出时自动更新，不调 LLM 生成（零额外 token）。
// 每轮注入 charter 作为「工作面板」，告诉 AI 手头有什么材料、做了哪些决策。
// 内容（字幕/正文/截图）在 artifact 里，需要时调 read_artifact 召回——不复述在 task_state。
// ============================================================

use crate::commands::tools::{ArtifactRef, Phase};

#[derive(Debug, Clone)]
pub struct TaskState {
    pub phase: Phase,
    /// 输入摘要（前 200 字符），让 agent 一眼知道在炼化什么
    pub input_summary: String,
    /// 已产出的 artifact 引用清单（工具结果落盘后的指针）
    pub artifact_refs: Vec<ArtifactRef>,
    /// 已做的关键决策（如「有字幕，已跳过 ASR」）
    pub decisions: Vec<String>,
    /// 待解决项（如「术语表待补全」）
    pub open_issues: Vec<String>,
}

impl TaskState {
    pub fn new(input: &str) -> Self {
        let summary: String = input.chars().take(200).collect();
        Self {
            phase: Phase::Acquire,
            input_summary: summary,
            artifact_refs: vec![],
            decisions: vec![],
            open_issues: vec![],
        }
    }

    /// 工具产出 artifact 时调用，记录引用。
    pub fn add_artifact(&mut self, art: ArtifactRef) {
        // 同 id 去重（如重复 read 同一文件）
        if !self.artifact_refs.iter().any(|a| a.id == art.id) {
            self.artifact_refs.push(art);
        }
    }

    /// 渲染为 YAML 块注入 charter（给 agent 看的工作面板）。
    pub fn to_yaml(&self) -> String {
        let arts: String = if self.artifact_refs.is_empty() {
            "  (尚无)".into()
        } else {
            self.artifact_refs
                .iter()
                .map(|a| {
                    format!(
                        "  - id: {}\n    kind: {}\n    tokens_estimate: {}\n    summary: {}\n    path: {}",
                        a.id,
                        a.kind.as_str(),
                        a.tokens_estimate,
                        a.summary,
                        a.path.display()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let decisions = if self.decisions.is_empty() {
            "  (无)".into()
        } else {
            self.decisions
                .iter()
                .map(|d| format!("  - {d}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "phase: {:?}\ninput: {}\nartifacts:\n{}\ndecisions:\n{}\n\
             note: 当你需要引用 artifact 的文件路径时，必须使用上面列出的绝对 path，禁止使用 temp_dir/...、./...、/tmp/... 等占位符。",
            self.phase, self.input_summary, arts, decisions
        )
        // 消毒反引号：charter.md 把 task_state_yaml 包在 ```yaml 围栏里，
        // 若内容含反引号会过早闭合围栏破坏 charter 结构（对抗审查 high 级 finding）。
        .replace('`', "'")
    }
}
