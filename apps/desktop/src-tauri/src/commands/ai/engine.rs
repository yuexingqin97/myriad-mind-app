// ============================================================
// MindEngine — 统一 AI 调用入口
// 职责: 读密钥链 → 调 DeepSeekClient → 返回响应
// ============================================================

use super::deepseek::stream_deepseek;
use super::types::{MindRequest, MindResponse};
use crate::error::AppError;
use tauri::AppHandle;

/// 从 OS 密钥链读取 DeepSeek API Key
pub fn read_deepseek_key() -> Result<String, AppError> {
    // 1. 尝试 OS 密钥链 (Windows Credential Manager)
    #[cfg(target_os = "windows")]
    {
        use crate::commands::config::cred_read;
        if let Ok(Some(key)) = cred_read("deepseek-api-key") {
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }

    // 2. 环境变量兜底
    for env_name in &["MYRIAD_DEEPSEEK_API_KEY", "DEEPSEEK_API_KEY"] {
        if let Ok(key) = std::env::var(env_name) {
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }

    Err(AppError::Ai {
        kind: "provider_not_configured".into(),
        message: "未找到 DeepSeek API Key。请在设置中配置，或设置环境变量 DEEPSEEK_API_KEY。"
            .into(),
    })
}

/// 运行 AI 任务 — Tauri command
#[tauri::command]
pub async fn run_mind_task(
    app_handle: AppHandle,
    request: MindRequest,
) -> Result<MindResponse, AppError> {
    let api_key = read_deepseek_key()?;
    stream_deepseek(&app_handle, &request, &api_key).await
}

/// 便捷函数：用 DeepSeek V4 Pro 生成学习笔记
/// 供 pipeline.rs 等内部模块调用
pub async fn generate_note(
    app_handle: &AppHandle,
    content: &str,
    content_type: &str,
    note_dir: Option<&str>,    // 输出目录，用于读取 memory.md
    task_prompt: Option<&str>, // 用户本次要求
) -> Result<String, AppError> {
    let api_key = read_deepseek_key()?;

    // 读取知识库记忆
    let memory_context = if let Some(dir) = note_dir {
        let memory_path = std::path::PathBuf::from(dir)
            .join(".myriad-mind")
            .join("memory.md");
        if memory_path.exists() {
            std::fs::read_to_string(&memory_path).unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // 构建 system prompt — 严格对齐 SKILL.md 步骤 7 模板
    let mut system_prompt = String::from(
        "你是一位专业的学习笔记整理专家。请基于用户提供的视频素材（标题/作者/时长/原始链接/字幕文本），\n\
        生成一份高质量的结构化学习笔记。严格按照以下格式和章节顺序输出。\n\
        \n\
        ## 输出格式（严格遵循，不要跳过任何章节）\n\
        \n\
        ### 标题\n\
        用一级标题：`# 原标题 — 学习笔记`\n\
        紧接着一个空行，然后来源行：`> 📺 来源：[平台名](原始链接) | 作者：XXX | 时长：XX分XX秒`\n\
        再一行：`> 💡 点击 ▶ 时间戳可跳转到视频对应位置`\n\
        再一个空行，然后阅读信息行和标签行（见下方格式）。\n\
        再用 `---` 分隔线。\n\
        \n\
        阅读信息行格式：`> 📖 推荐阅读时长：XX 分钟 | 难度：🌿 进阶 | 可靠性：🟡 参考`\n\
        标签行格式：`> 🏷️ #Tag1 #Tag2 #Tag3`（3-6 个标签）\n\
        \n\
        ### 一、AI 摘要\n\
        2-3 段，200-300 字。概括核心内容、覆盖的主要主题、适合什么水平的读者。\n\
        要具体不要虚（提到实际的技术名词和关键论点）。\n\
        \n\
        ### 二、核心概念\n\
        列出 3-5 个最重要的概念。每个概念格式：`1. **概念名** — 一段话解释（2-3句）`\n\
        \n\
        ### 三、详细笔记\n\
        按内容逻辑分段，每段 8-12 个小节。每段格式必须为：\n\
        `### [▶ MM:SS](原始链接?t=总秒数) - MM:SS | 段落标题`\n\
        然后写正文（不要只写一两句话，要提炼关键论证、步骤、代码要点、注意事项）。\n\
        正文下方如有截图，用 `![说明](assets/VIDEO_ID/frame_XXXX.png)` 嵌入。\n\
        截图下方配可点击时间戳：`> 📸 [截图于 M:SS](原始链接?t=秒数)`\n\
        总秒数 = M×60 + S，如实计算。\n\
        如果发现过时/错误内容，用 `> ⚠️ **版本差异**：XXX` 标注。\n\
        \n\
        ### 四、关键术语表\n\
        | 术语 | 中文 | 简要说明 |\n\
        | --- | --- | --- |\n\
        | Entity | 实体 | 唯一 ID，本身不存储数据 |\n\
        至少 8 个术语，每个一行表格。\n\
        \n\
        ### 五、知识关系图\n\
        用二级标题 `## 五、知识关系图`。空一行，然后 **图：XXX知识关系图**。再空一行，然后 mermaid 代码块。\n\
        代码块第一行：`%%{init: {'theme': 'dark'}}%%`\n\
        内容结构：\n\
        - 中心节点 = 主题（🎯 emoji）\n\
        - 一级分支 = 核心概念（3-5 个）\n\
        - 二级分支 = 关键知识点\n\
        - 虚线(-.->)连接相关概念\n\
        - 标注前置知识（📥）和扩展方向（📤）\n\
        节点标签不要包含 `|` 字符，用 `·` 替代。\n\
        \n\
        ### 六、扩展学习资源\n\
        5-8 个资源，按「📖 官方文档」「🐙 GitHub 仓库」「📚 延伸阅读」分三组。\n\
        每个链接配 10-20 字推荐理由。基于训练数据推荐真实存在的资源，不要编造链接。\n\
        \n\
        ### 七、总结与思考\n\
        2-3 句话总结核心价值 + 学习路径建议。\n\
        \n\
        ### 文档末尾元信息\n\
        用 `---` 分隔线隔开后，添加元信息引用块：\n\
        > 📋 **文档元信息**\n\
        > | 文档版本 | v1.0 |\n\
        > | 生成时间 | {当前时间} |\n\
        > | 内容类型 | 视频 |\n\
        > | AI 建议分类 | Rust |\n\
        > ⚡ 本文档由 AI 自动生成。字幕来自语音识别，可能存在同音字或断句误差，建议结合原视频对照学习。\n\
        \n\
        AI 建议分类 必须填写。根据内容主题选择一个最合适的分类名（如 Rust / AI / 前端 / 后端 / 游戏开发 / DevOps / Python / CS基础 等），用于自动归档到对应目录。\n\
        \n\
        ## 重要规则\n\
        \n\
        ### 时间戳链接\n\
        所有时间戳必须生成平台对应的可点击跳转链接。用户提供原始链接后，根据域名选择格式：\n\
        - B 站：`(https://www.bilibili.com/video/BVxxx/?t=总秒数)`\n\
        - YouTube：`(https://www.youtube.com/watch?v=ID&t=总秒数)`\n\
        - 本地文件：`(file:///路径?t=总秒数)`\n\
        不要用占位符。总秒数 = MM×60 + SS。\n\
        \n\
        ### Mermaid 图表\n\
        - 知识关系图是必须的（至少 1 张）\n\
        - 详细笔记中如有架构/流程/时序关系，也主动绘制 Mermaid 图\n\
        - 图表紧跟在相关文字之后\n\
        - 每个图有加粗标题（如 **图：ECS 架构层级关系**）\n\
        - 所有 Mermaid 图第一行添加 `%%{init: {'theme': 'dark'}}%%`\n\
        \n\
        ### 截图使用\n\
        - 如果用户提供了「截图审查结果」，按审查表中标注的嵌入位置放置截图\n\
        - 每张截图放在对应知识点的正下方\n\
        - 截图下方必须配可点击时间戳\n\
        \n\
        ### 教程模式\n\
        - 如果用户标注了「本视频被检测为操作型教程」，额外生成 **📋 操作流程总览**（Mermaid flowchart）\n\
        - 每个节点用 `▶ 时间戳` 标注，用 `click` 语法链接到视频对应时间\n\
        \n\
        ### 阅读信息计算\n\
        `阅读时长 = (基础分钟 + 图表秒 + 代码秒) × 难度系数`\n\
        - 中文阅读基准：400 字/分钟\n\
        - 每个 Mermaid 图 +15s，每张截图 +10s，代码每行 +2s\n\
        - 难度系数：🌱 入门 ×1.0 / 🌿 进阶 ×1.3 / 🌳 深入 ×1.6\n\
        - 结果四舍五入，最少标 1 分钟\n\
        \n\
        ### 难度评级\n\
        🌱 入门（零基础） / 🌿 进阶（需一定基础，涉及实现细节） / 🌳 深入（源码级密度，底层原理）\n\
        \n\
        ### 可靠性评级\n\
        🟢 可信（与官方文档一致） / 🟡 参考（大部分正确，少量过时） / 🟠 谨慎（有争议） / 🔴 仅作了解\n\
        如发现过时/错误内容，用引用块标注：`> ⚠️ **版本差异**：XXX`\n\
        \n\
        ### 内容要求\n\
        - 不要只写简短摘要。把字幕中的关键论证、步骤、代码要点、注意事项炼化成可复习的正文。\n\
        - 每个「详细笔记」小节至少写 3-5 句正文，不是一行标题就算完。\n\
        - 如果素材是英文，自动翻译并以中文讲解；必要时保留关键英文原句或术语对照。\n\
        - 对不确定、过时或无法从素材确认的信息明确标注，不要编造。\n\
        \n\
        ### 输出语言\n\
        中文。专业术语保留英文原名（括号标注）。代码块保留原文。",
    );

    // 注入知识库记忆
    system_prompt.push_str(
        "\n\n## Mermaid 代码块硬性格式\n\n\
        每一张 Mermaid 图必须是一个完整 fenced code block，格式只能如下：\n\n\
        ```mermaid\n\
        %%{init: {'theme': 'dark'}}%%\n\
        graph TD\n\
          core[🎯 ECS 核心架构] --> component[Component]\n\
        ```\n\n\
        禁止把 `%%{init...}%%`、`graph TD`、`flowchart TD` 放在代码块外面。\n\
        禁止先输出 Mermaid 内容再补 ```；禁止把标题、解释文字放进 Mermaid 代码块。\n\
        Mermaid 节点文本优先使用 ASCII ID + 方括号标签，例如 `core[🎯 ECS 核心架构]`，不要直接把 emoji 或中文作为节点 ID。\n",
    );

    system_prompt.push_str(
        "\n\n## AI 分类要求\n\n\
        你必须根据整篇内容主题给出一个“AI 建议分类”，用于应用保存目录。\n\
        分类名由你判断，不要局限于固定列表；优先简短、稳定、可复用，例如 `Rust`、`Bevy`、`AI Agent`、`前端工程`、`系统设计`。\n\
        分类名只能使用中文、英文、数字、空格、短横线、下划线或 `·`，不要包含 `/ \\ : * ? \" < > |` 等文件路径非法字符。\n\
        在“文档末尾元信息”的表格中必须添加两行：`> | AI 建议分类 | 分类名 |` 和 `> | ai_category | 分类名 |`。\n",
    );

    system_prompt.push_str(&content_mode_prompt(content_type));

    if !memory_context.is_empty() {
        system_prompt.push_str(&format!(
            "\n\n## 当前知识库已有内容（供参考，避免重复）\n\n{memory_context}\n\n\
            注意：如果本次内容与已有笔记相似或重复，优先更新已有笔记而非创建重复内容。"
        ));
    }

    // 注入本次要求
    if let Some(prompt) = task_prompt {
        if !prompt.trim().is_empty() {
            system_prompt.push_str(&format!(
                "\n\n## 用户本次特别要求\n\n{prompt}\n\n请严格遵守以上要求。"
            ));
        }
    }

    system_prompt.push_str(&format!("\n\n内容类型：{content_type}"));

    let req = MindRequest {
        task: super::types::AiTask::NoteGeneration,
        messages: vec![super::types::AiMessage {
            role: "user".into(),
            content: content.to_string(),
        }],
        system_prompt,
        model_override: None,
        stream: true,
        max_tokens: Some(65536),
        thinking: Some(super::types::ThinkingConfig {
            enabled: true,
            effort: Some(super::types::ReasoningEffort::High),
        }),
    };

    let resp = stream_deepseek(app_handle, &req, &api_key).await?;
    log::info!("[mind-engine] generated {} chars", resp.text.len());
    Ok(resp.text)
}

/// 对已有笔记进行追问，返回答案
fn content_mode_prompt(content_type: &str) -> String {
    let common = "\n\n## 跨格式质量要求\n\n\
        不论输入是视频、网页、文档还是代码，都必须达到 Skill 版学习笔记质量：\n\
        - 输出中文学习笔记，技术术语保留英文原名并给中文解释。\n\
        - 必须包含：AI 摘要、核心要点、详细笔记、关键术语表、知识关系图、扩展学习资源、总结与思考、文档末尾元信息。\n\
        - 必须给阅读信息：推荐阅读时长、难度、可靠性、3-6 个标签。\n\
        - 信息不足时明确标注“不确定/材料未覆盖”，不要编造。\n\
        - Mermaid 至少 1 张；代码/架构/流程内容应主动增加架构图、流程图或时序图。\n";

    let mode = if content_type.contains("代码") {
        "\n\n## 代码项目输出契约\n\n\
        输入材料包含目录结构和关键文件片段。请生成代码项目学习笔记，重点包括：\n\
        1. 项目一句话定位、技术栈、运行/构建入口。\n\
        2. 目录结构解读：按模块解释职责，不要逐文件流水账。\n\
        3. 核心架构：组件关系、数据流、关键调用链，至少一张 Mermaid 架构图。\n\
        4. 关键文件导读：列出 8-15 个最值得读的文件，说明为什么读、先后顺序。\n\
        5. 代码机制详解：从材料中提取真实函数/类型/配置名，解释实现思路和扩展点。\n\
        6. 风险与改进建议：测试、错误处理、性能、边界条件、可维护性。\n\
        7. 学习路线：读者接下来应该补哪些前置知识、看哪些官方资源。\n"
    } else if content_type.contains("网页") || content_type.contains("文章") {
        "\n\n## 网页/文章输出契约\n\n\
        输入材料来自网页正文。请生成文章学习笔记，重点包括：\n\
        1. 原文主张与适用对象，区分作者观点、事实、推论。\n\
        2. 核心论点 3-6 条，每条给解释和应用场景。\n\
        3. 详细笔记按原文逻辑分段，保留关键术语、数据、例子。\n\
        4. 如果文章是教程，补充步骤流程图；如果是观点文，补充论证结构图。\n\
        5. 给出可验证的延伸资源；不确定真实存在的链接不要编造。\n"
    } else if content_type.contains("文档") {
        "\n\n## 本地文档输出契约\n\n\
        输入材料来自本地文档或 Markdown。请保留原文结构的同时重新炼化为学习笔记：\n\
        1. 识别文档类型：教程、规范、会议记录、设计文档、读书笔记或代码说明。\n\
        2. 保留重要标题层级，但重写为可复习的知识结构。\n\
        3. 提取定义、流程、约束、决策、TODO、风险和开放问题。\n\
        4. 对长文档给“快速阅读路线”和“深入阅读路线”。\n"
    } else {
        "\n\n## 视频输出契约\n\n\
        输入材料来自视频字幕、元信息、关键帧和截图审查。请对齐 Skill 版视频笔记：\n\
        1. 段落标题尽量带可点击时间戳；没有时间戳时按主题分段。\n\
        2. 把字幕中的口语化内容炼化为可复习正文，修正常见 ASR 断句/同音错误。\n\
        3. 如有截图审查结果，把关键画面嵌入到对应知识点下，并说明画面价值。\n\
        4. 检测教程/演示内容时，额外生成操作流程总览 Mermaid。\n"
    };

    format!("{common}{mode}")
}

pub async fn qa_note(
    app_handle: &AppHandle,
    note_content: &str,
    question: &str,
) -> Result<String, AppError> {
    let api_key = read_deepseek_key()?;
    let system_prompt = format!(
        "你是一个学习助手。基于以下笔记内容回答用户问题。\n\
        回答要简洁、结构化，引用笔记中的具体章节。\n\n\
        笔记内容：\n{note_content}"
    );

    let req = MindRequest {
        task: super::types::AiTask::NoteGeneration,
        messages: vec![super::types::AiMessage {
            role: "user".into(),
            content: question.to_string(),
        }],
        system_prompt,
        model_override: Some("deepseek-v4-flash".into()),
        stream: true,
        max_tokens: Some(4096),
        thinking: None,
    };

    let resp = stream_deepseek(app_handle, &req, &api_key).await?;
    Ok(resp.text)
}

/// 测试 DeepSeek 连接
#[tauri::command]
pub async fn test_deepseek_connection(app_handle: AppHandle) -> Result<String, AppError> {
    let api_key = read_deepseek_key()?;
    let req = MindRequest {
        task: super::types::AiTask::Summary,
        messages: vec![super::types::AiMessage {
            role: "user".into(),
            content: "回复\"pong\"".into(),
        }],
        system_prompt: "回复 pong，不要回复其他内容。".into(),
        model_override: Some("deepseek-v4-flash".into()),
        stream: false,
        max_tokens: Some(10),
        thinking: None,
    };

    let response = stream_deepseek(&app_handle, &req, &api_key).await?;
    Ok(format!("pong — {}", response.model))
}
