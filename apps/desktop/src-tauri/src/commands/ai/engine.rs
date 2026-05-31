// ============================================================
// MindEngine — 统一 AI 调用入口
// 职责: 读密钥链 → 调 DeepSeekClient → 返回响应
// ============================================================

use super::deepseek::stream_deepseek;
use super::types::{MindRequest, MindResponse};
use crate::error::AppError;
use tauri::AppHandle;

/// 从多处读取 DeepSeek API Key（优先级：环境变量 > 配置文件 > OS 密钥链）
pub fn read_deepseek_key() -> Result<String, AppError> {
    // 1. 环境变量（最高优先级，CI/容器场景）
    for env_name in &["MYRIAD_DEEPSEEK_API_KEY", "DEEPSEEK_API_KEY"] {
        if let Ok(key) = std::env::var(env_name) {
            if !key.is_empty() {
                return Ok(key);
            }
        }
    }

    // 2. 配置文件 ~/myriad-mind-config.json
    if let Some(key) = crate::commands::config::read_config_value("deepseek_api_key") {
        return Ok(key);
    }

    // 3. OS 密钥链 (Windows Credential Manager)
    #[cfg(target_os = "windows")]
    {
        use crate::commands::config::cred_read;
        if let Ok(Some(key)) = cred_read("deepseek-api-key") {
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

    // 构建 system prompt — 根据内容类型条件化
    let is_video = content_type.contains("视频");
    let is_code = content_type.contains("代码") || content_type.contains("code");
    let is_article = content_type.contains("网页") || content_type.contains("文章");
    let _is_document = content_type.contains("文档") || content_type.contains("文本");

    let mut system_prompt = String::from(
        "你是一位专业的学习笔记整理专家。请基于用户提供的素材，\n\
        生成一份高质量的结构化学习笔记。严格按照以下格式和章节顺序输出。\n\
        \n\
        ## 输出格式（严格遵循，不要跳过任何章节）\n\
        \n\
        ### 标题\n\
        用一级标题：`# 原标题 — 学习笔记`\n\
        紧接着一个空行，然后来源行。来源行格式根据内容类型：\n"
    );

    // 条件化来源行格式
    if is_video {
        system_prompt.push_str(
            "- `> 📺 来源：[平台名](原始链接) | 作者：XXX | 时长：XX分XX秒`\n\
             - `> 💡 点击 ▶ 时间戳可跳转到视频对应位置`\n",
        );
    } else if is_article {
        system_prompt.push_str(
            "- `> 📺 来源：[平台名](URL) | 作者：XXX | 发布日期：YYYY-MM-DD`\n\
             - `> 💡 原文可点击链接查看`\n",
        );
    } else if is_code {
        system_prompt.push_str(
            "- `> 📂 原始资源：{路径或 URL}`\n\
             - `> ⚠️ 本报告由 AI 基于代码阅读生成，可能存在理解偏差。建议搭配源码阅读使用。`\n",
        );
    } else {
        system_prompt.push_str(
            "- `> 📂 原始资源：{文件路径}`\n\
             - `> 📅 文件类型：本地 Markdown / 文本文档`\n",
        );
    }

    system_prompt.push_str(
        "再一个空行，然后阅读信息行和标签行（见下方格式）。\n\
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
        \n",
    );

    // 条件化详细笔记格式
    if is_video {
        system_prompt.push_str(
            "### 三、详细笔记\n\
            按内容逻辑分段，每段 8-12 个小节。每段格式必须为：\n\
            `### [▶ MM:SS](原始链接?t=总秒数) - MM:SS | 段落标题`\n\
            然后写正文（不要只写一两句话，要提炼关键论证、步骤、代码要点、注意事项）。\n\
            正文下方如有截图，用 `![说明](assets/VIDEO_ID/frame_XXXX.png)` 嵌入。\n\
            截图下方配可点击时间戳：`> 📸 [截图于 M:SS](原始链接?t=秒数)`\n\
            总秒数 = M×60 + S，如实计算。\n\
            如果发现过时/错误内容，用 `> ⚠️ **版本差异**：XXX` 标注。\n\
            \n",
        );
    } else {
        system_prompt.push_str(
            "### 三、详细笔记\n\
            按内容逻辑分段。段落标题格式：`### 段落主题`（不需要时间戳和 ▶ 符号）。\n\
            正文要提炼关键论证、步骤、代码要点、注意事项，不要只写一两句话。\n\
            如果发现过时/错误内容，用 `> ⚠️ **版本差异**：XXX` 标注。\n\
            \n",
        );
    }

    system_prompt.push_str(
        "### 四、关键术语表\n\
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
        - **每个节点必须有 ASCII 节点 ID**：写成 `n1[🎯 标签] --> n2[📦 标签]`，**严禁**写成 `-->[标签]` 不带 ID。节点 ID 用英文字母+数字（如 n1,f1,c1），标签放方括号内。\n\
        节点标签不要包含 `|` 字符，用 `·` 替代。\n\
        \n\
        ### 六、扩展学习资源\n\
        5-8 个资源，按「📖 官方文档」「🐙 GitHub 仓库」「📚 延伸阅读」分三组。\n\
        每个链接配 10-20 字推荐理由。基于训练数据推荐真实存在的资源，不要编造链接。\n\
        \n\
        ### 七、总结与思考\n\
        2-3 句话总结核心价值 + 学习路径建议。\n\
        \n\
        ## 重要规则\n\
        \n",
    );

    // 条件化：仅视频模式注入时间戳/截图/教程规则
    if is_video {
        system_prompt.push_str(
            "### 时间戳链接\n\
            所有时间戳必须生成平台对应的可点击跳转链接。用户提供原始链接后，根据域名选择格式：\n\
            - B 站：`(https://www.bilibili.com/video/BVxxx/?t=总秒数)`\n\
            - YouTube：`(https://www.youtube.com/watch?v=ID&t=总秒数)`\n\
            - 本地文件：`(file:///路径?t=总秒数)`\n\
            不要用占位符。总秒数 = MM×60 + SS。\n\
            \n\
            ### 截图使用\n\
            - 如果用户提供了「截图审查结果」，按审查表中标注的嵌入位置放置截图\n\
            - 每张截图放在对应知识点的正下方\n\
            - 截图下方必须配可点击时间戳\n\
            \n\
            ### 教程模式\n\
            - 如果用户标注了「本视频被检测为操作型教程」，额外生成 **📋 操作流程总览**（Mermaid flowchart）\n\
            - 每个节点用 `▶ 时间戳` 标注，用 `click` 语法链接到视频对应时间\n\
            \n",
        );
    }

    system_prompt.push_str(
        "### Mermaid 图表\n\
        - 知识关系图是必须的（至少 1 张）\n\
        - 详细笔记中如有架构/流程/时序关系，也主动绘制 Mermaid 图\n\
        - 图表紧跟在相关文字之后\n\
        - 每个图有加粗标题（如 **图：ECS 架构层级关系**）\n\
        - 所有 Mermaid 图第一行添加 `%%{init: {'theme': 'dark'}}%%`\n\
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
        - 不要只写简短摘要。把素材中的关键论证、步骤、代码要点、注意事项炼化成可复习的正文。\n\
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
          core[🎯 ECS 核心架构] --> c1[Component]\n\
          c1 --> s1[System]\n\
        ```\n\n\
        **⚠️ 每个节点必须有 ASCII 节点 ID（如 n1、c1、f1），严禁直接写 `[标签]` 不带 ID。**\n\
        错误示例：`-->[📦 Future]`（缺少节点 ID，Mermaid 解析失败）\n\
        正确示例：`--> f1[📦 Future·惰性求值]`\n\n\
        禁止把 `%%{init...}%%`、`graph TD`、`flowchart TD` 放在代码块外面。\n\
        禁止先输出 Mermaid 内容再补 ```；禁止把标题、解释文字放进 Mermaid 代码块。\n\
        Mermaid 节点文本优先使用 ASCII ID + 方括号标签，例如 `core[🎯 ECS 核心架构]`，不要直接把 emoji 或中文作为节点 ID。\n",
    );

    system_prompt.push_str(
        "\n\n## AI 分类要求\n\n\
        你必须根据整篇内容主题给出一个「AI 建议分类」，用于应用保存目录。\n\
        分类名由你判断，不要局限于固定列表；优先简短、稳定、可复用，例如 `Rust`、`Bevy`、`AI Agent`、`前端工程`、`系统设计`。\n\
        分类名只能使用中文、英文、数字、空格、短横线、下划线或 `·`，不要包含 `/ \\ : * ? \" < > |` 等文件路径非法字符。\n\
        在笔记末尾（总结与思考之后）单独一行输出：`> ai_category: 分类名`\n",
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
    // 去掉 "更新·" 前缀用于匹配
    let ct = content_type.trim_start_matches("更新·");

    let common = "\n\n## 跨格式质量要求\n\n\
        不论输入是视频、网页、文档还是代码，生成的学习笔记必须达到以下标准：\n\
        - 输出中文学习笔记，技术术语保留英文原名并给中文解释。\n\
        - 必须包含：AI 摘要、核心要点、详细笔记、关键术语表、知识关系图、扩展学习资源、总结与思考、文档末尾元信息。\n\
        - 必须给阅读信息：推荐阅读时长、难度、可靠性、3-6 个标签。\n\
        - 信息不足时明确标注「不确定/材料未覆盖」，不要编造。\n\
        - Mermaid 至少 1 张；代码/架构/流程内容应主动增加架构图、流程图或时序图。\n";

    let mode = if ct.contains("代码") || ct.contains("code") {
        "\n\n## 代码项目输出契约\n\n\
        输入材料包含目录结构和关键文件片段。请生成代码项目学习笔记，严格包含以下章节：\n\n\
        1. **项目概览** — 一句话定位、技术栈、运行/构建入口点。用表格呈现项目信息（名称/语言/构建工具/规模/许可证）。\n\
        2. **目录结构解读** — 按模块解释职责，不要逐文件流水账。用代码块展示 tree 结构。\n\
        3. **核心架构** — 组件关系、数据流、关键调用链。必须包含至少两张 Mermaid 图：\n\
           - 架构总图（graph TD，展示模块层级和依赖）\n\
           - 数据流图（flowchart LR 或 sequenceDiagram，展示数据如何流经系统）\n\
        4. **关键文件导读** — 列出 8-15 个最值得读的文件，每个标注：\n\
           - 为什么读（一句话）\n\
           - 推荐阅读顺序（第几步读）\n\
           - 文件角色（入口/配置/核心/辅助）\n\
        5. **代码机制详解** — 从材料中提取真实函数名/类型名/配置项，解释：\n\
           - 实现思路和关键设计决策\n\
           - 扩展点和可配置项\n\
           - 与其他模块的交互方式\n\
        6. **风险与改进建议** — 从以下维度评估：\n\
           - 测试覆盖（有测试吗？测试什么？）\n\
           - 错误处理（异常路径是否覆盖？）\n\
           - 性能（有无明显瓶颈？）\n\
           - 边界条件和可维护性\n\
        7. **学习路线** — 读者接下来应该：\n\
           - 补哪些前置知识\n\
           - 看哪些官方资源\n\
           - 按什么顺序深入\n\
        8. **知识关系图** — Mermaid 图展示项目内核心概念及其关联结构。\n\n\
        **重要规则：**\n\
        - 不要包含视频时间戳（这是代码项目，不是视频）。\n\
        - 来源行格式：`> 📂 原始资源：{路径或 URL}`\n\
        - 可靠性评级基于：代码新鲜度、文档质量、测试覆盖。\n\
        - 保留所有真实的函数名、类型名、模块名，不要编造或改名。\n\
        - 标签维度：技术栈(#Rust) / 框架(#Bevy) / 内容类型(#源码分析) / 难度(#进阶)\n"
    } else if ct.contains("网页") || ct.contains("文章") {
        "\n\n## 网页/文章输出契约\n\n\
        输入材料来自网页正文。请生成文章学习笔记，严格包含以下章节：\n\n\
        1. **原文主张与适用对象** — 区分作者观点 vs 事实 vs 推论。\n\
        2. **核心论点 (3-6 条)** — 每条包含：一句话概括、详细解释、应用场景。\n\
        3. **详细笔记** — 按原文段落/逻辑分段组织：\n\
           - 保留关键术语、数据、例子\n\
           - 如果原文有代码，保留代码块\n\
           - 不标注时间戳（文章无视频时间轴）\n\
        4. **内容结构图** — 根据文章类型选择：\n\
           - 教程类文章：Mermaid flowchart 展示步骤流程\n\
           - 观点类文章：Mermaid graph 展示论证结构（论点→论据→结论）\n\
           - 技术类文章：Mermaid graph 展示概念关系\n\
        5. **扩展学习资源** — 可验证的外部链接：\n\
           - 官方文档 > 知名博客 > 个人博客\n\
           - 不确定真实存在的链接不要编造\n\
           - 每条配 10-20 字推荐理由\n\n\
        **文章专用规则：**\n\
        - 不要包含视频时间戳（这是文章，不是视频）。\n\
        - 不要包含截图嵌入指令（文章图片直接用 URL）。\n\
        - 来源行格式：`> 📺 来源：[平台名](URL) | 作者：XXX | 发布日期：YYYY-MM-DD`\n\
        - 可靠性评级额外考虑：\n\
          - 来源权威性（官方文档 > 知名博客 > 个人博客 > 匿名）\n\
          - 时效性（发布日期距今多久？内容是否过时？）\n\
          - 引用链（文章是否引用了官方来源/论文/源码？）\n\
        - 文中图片用 `![描述](URL)` 直接引用，不下载。\n\
        - 标签维度：领域(#前端) / 主题(#性能优化) / 内容类型(#教程) / 难度(#进阶)\n"
    } else if ct.contains("文档") || ct.contains("文本") {
        "\n\n## 本地文档输出契约\n\n\
        输入材料来自本地文档或 Markdown 文件。请保留原文结构的同时重新炼化为学习笔记：\n\n\
        1. **文档类型识别** — 判断原文是：教程、规范、会议记录、设计文档、读书笔记、代码说明等。\n\
        2. **结构重写** — 保留重要标题层级，但重写为可复习的知识结构：\n\
           - 每个章节提炼为 3-5 个可复习的要点\n\
           - 提取定义、流程、约束、决策、TODO、风险和开放问题\n\
        3. **阅读路线** — 对长文档给出两条路线：\n\
           - 快速路线（5 分钟）：只看核心概念和结论\n\
           - 深入路线（完整）：逐节精读，理解每个细节\n\
        4. **保留原文要素**：\n\
           - 代码块保留原文\n\
           - 表格保留结构\n\
           - 关键原文引用用 `> 引用块` 标注\n\n\
        **文档专用规则：**\n\
        - 不要包含视频时间戳。\n\
        - 不要包含截图嵌入指令。\n\
        - 来源行格式：`> 📂 原始资源：{文件路径} | 文件类型：{Markdown/文本/PDF}`\n\
        - 原始文件路径必须出现在笔记头部。\n\
        - 如果原文是英文，翻译关键内容为中文，保留重要英文术语。\n\
        - 标签维度：领域 / 主题 / 内容类型 / 难度\n"
    } else if ct.contains("短文") {
        "\n\n## 短文输出契约\n\n\
        输入材料较短（< 500 字）。请精炼为学习卡片式笔记：\n\
        1. 一句话总结核心内容。\n\
        2. 提取 2-3 个关键要点。\n\
        3. 如有术语，给出简要解释。\n\
        4. 不需要完整的 8 章结构，保持简洁。\n"
    } else {
        // 视频模式（默认）
        "\n\n## 视频输出契约\n\n\
        输入材料来自视频字幕、元信息、关键帧和截图审查。请生成视频学习笔记：\n\
        1. 段落标题必须带可点击时间戳链接：`### [▶ MM:SS](链接?t=秒数) - MM:SS | 段落标题`\n\
        2. 把字幕中的口语化内容炼化为可复习正文，修正常见 ASR 断句/同音错误。\n\
        3. 如有截图审查结果，把关键画面嵌入到对应知识点下，并说明画面价值。\n\
        4. 检测教程/演示内容时，额外生成 📋 操作流程总览（Mermaid flowchart）。\n"
    };

    format!("{common}{mode}")
}

#[allow(dead_code)]
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
