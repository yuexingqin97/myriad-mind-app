// ============================================================
// Prompt 模板 — 学习笔记生成（最核心 Prompt）
// 与 SKILL.md 步骤 7 对齐
// ============================================================

import type { InputMode, NoteDifficulty } from "../types.js";

export interface NoteGenContext {
  // 基本元信息
  title: string;
  author?: string;
  duration?: string; // 视频时长
  platform: string;
  sourceUrl: string;
  mode: InputMode;

  // 内容
  textContent: string; // 字幕/正文
  aiSummary: string;

  // 可选
  hasKeyframes?: boolean;
  keyframeList?: Array<{ file: string; timestamp: string }>;
  hasComments?: boolean;

  // 功能开关
  enableKeyframes: boolean;
  enableMermaid: boolean;
  enableResources: boolean;
  enableComments: boolean;
  enableReadingInfo: boolean;
}

/**
 * 构建视频学习笔记生成 Prompt
 */
export function buildVideoNotePrompt(ctx: NoteGenContext): string {
  const baseLines: Array<string | null> = [
    `请基于以下视频素材生成一份结构化学习笔记：`,
    ``,
    `视频标题：${ctx.title}`,
    ctx.author ? `作者：${ctx.author}` : null,
    ctx.duration ? `时长：${ctx.duration}` : null,
    `来源：${ctx.platform}`,
    `原始链接：${ctx.sourceUrl}`,
    ``,
    `字幕文本（已翻译/原文）：`,
    ctx.textContent,
    ``,
    `AI 摘要：`,
    ctx.aiSummary,
  ];

  const keyframeLines: Array<string | null> = [];
  if (ctx.hasKeyframes && ctx.enableKeyframes && ctx.keyframeList) {
    keyframeLines.push(
      ``,
      `关键帧截图（${ctx.keyframeList.length} 张）：`,
      ...ctx.keyframeList.map((f) => `- ${f.file} (${f.timestamp})`)
    );
  }

  const sectionLines: Array<string | null> = [
    ``,
    `请生成以下内容：`,
    ctx.enableReadingInfo
      ? `0. **阅读信息**：推荐阅读时长 + 难度评级（见下方说明）`
      : null,
    `1. 核心概念（3-5 个最重要的概念）`,
    `2. 详细笔记（按内容逻辑分段，每段标注时间范围，标题格式：### [▶ MM:SS](链接?t=秒数) - MM:SS | 段落标题）`,
    ctx.enableKeyframes ? `3. 关键画面描述（分析截图内容，标注时间点）` : null,
    `4. 关键术语表（英文术语 → 中文翻译 → 简要说明）`,
    `5. 总结与思考`,
    ctx.enableResources
      ? `6. 扩展学习资源（推荐进一步学习的方向和链接）`
      : null,
    ctx.enableComments
      ? `7. 评论区精华讨论（精选 3-6 条有价值的评论，附跳转链接；无高质量评论则跳过）`
      : null,
    ctx.enableMermaid
      ? `8. 知识关系图（Mermaid 图，展示本课核心概念及其关联结构）`
      : null,
    ...getNoteGenFormatRules(ctx),
  ];

  return [...baseLines, ...keyframeLines, ...sectionLines]
    .filter((s) => s != null)
    .join("\n");
}

/**
 * 构建文章笔记生成 Prompt
 */
export function buildArticleNotePrompt(ctx: NoteGenContext): string {
  return [
    `请基于以下文章内容生成一份结构化学习笔记：`,
    ``,
    `文章标题：${ctx.title}`,
    ctx.author ? `作者：${ctx.author}` : null,
    `来源：${ctx.platform}`,
    `原文链接：${ctx.sourceUrl}`,
    ``,
    `文章正文：`,
    ctx.textContent,
    ``,
    `请生成以下内容：`,
    ctx.enableReadingInfo
      ? `0. 阅读信息：推荐阅读时长 + 难度评级（见下方说明）`
      : null,
    `1. 核心概念（3-5 个最重要的概念）`,
    `2. 详细笔记（按文章段落/章节逻辑分段，不标注时间）`,
    `3. 关键术语表（英文术语 → 中文翻译 → 简要说明）`,
    `4. 总结与思考`,
    ctx.enableResources
      ? `5. 扩展学习资源：官方文档、相关文章、GitHub 仓库等`
      : null,
    ...getArticleFormatRules(ctx),
  ]
    .filter((s): s is string => s != null)
    .join("\n");
}

// ---- 共享格式规则 ----

function getNoteGenFormatRules(
  ctx: NoteGenContext
): Array<string | null> {
  return [
    `**阅读时长计算（三步法）：**`,
    ``,
    `第一步：基础阅读时间 — 中文 400 字/分钟，统计正文总字数（不含代码、图表），除以 400`,
    `第二步：图表浏览时间 — 每个 Mermaid 图 +15秒，每张截图 +10秒，每个代码块按行数×2秒`,
    `第三步：难度系数修正 — 🌱入门 ×1.0 | 🌿进阶 ×1.3 | 🌳深入 ×1.6`,
    `公式：(基础分钟 + 图表分钟 + 代码分钟) × 难度系数，四舍五入取整，最少 1 分钟`,
    ``,
    `**难度评级标准：**`,
    `⭐ 入门 🌱 — 面向零基础，无需前置知识`,
    `⭐⭐ 进阶 🌿 — 需要一定基础，涉及具体实现或原理`,
    `⭐⭐⭐ 深入 🌳 — 面向有经验者，涉及源码解读、底层机制`,
    ``,
    `**内容可靠性评估：**`,
    `🟢 可信 — 与官方文档一致 | 🟡 参考 — 大部分正确 | 🟠 谨慎 — 有争议内容 | 🔴 仅作了解`,
    ``,
    `**输入格式：**`,
    `- 笔记开头添加：> 📖 推荐阅读时长：XX 分钟 | 难度：🌿 进阶 | 可靠性：🟡 参考`,
    `- 下一行添加标签：> 🏷️ #Rust #Bevy #ECS #源码分析`,
    `- 自动提取 3-6 个标签（技术栈/框架/主题/类型/难度维度）`,
    ``,
    ctx.enableMermaid ? `**Mermaid 图表规则：**` : null,
    ctx.enableMermaid
      ? `- 主动绘制 Mermaid 图表：架构关系用 graph TD、流程图用 flowchart、时序用 sequenceDiagram`
      : null,
    ctx.enableMermaid
      ? `- 每个核心概念至少配一个图，图表紧跟相关文字之后`
      : null,
    ctx.enableMermaid
      ? `- 截图展示视频画面，Mermaid 展示抽象关系，两者互补不重复`
      : null,
    ctx.enableMermaid
      ? `- 少于 3 个节点的图用文字描述即可`
      : null,
    ``,
    `**时间戳规则：**`,
    `- 所有时间戳做成 Markdown 可点击链接 [MM:SS](源链接?t=总秒数)`,
    `- B 站 t= 后跟秒数，YouTube /本地文件同理`,
    ``,
    `**输出为完整 Markdown 文档，不含 JSON 包裹。**`,
  ];
}

function getArticleFormatRules(
  ctx: NoteGenContext
): Array<string | null> {
  return [
    ``,
    `**格式要求：**`,
    `- 不生成时间戳链接（文章无视频时间轴）`,
    `- 在开头标注阅读时长和难度`,
    ctx.enableMermaid ? `- 主动绘制 Mermaid 图表` : null,
    `- 在末尾附加扩展学习资源`,
    `- 章节用 "##" 编号，段落标题直接用 "### 段落主题"`,
    `- 输出为完整 Markdown 文档，不含 JSON 包裹`,
  ];
}

// ---- 阅读信息计算工具 ----

/**
 * 统计 Markdown 文档的纯文本字数（不含代码块、Mermaid 图表、表格）
 */
export function countNoteWords(markdown: string): number {
  let text = markdown
    // 移除代码块
    .replace(/```[\s\S]*?```/g, "")
    // 移除 Mermaid 块
    .replace(/```mermaid[\s\S]*?```/g, "")
    // 移除表格
    .replace(/\|.+\|/g, "")
    // 移除 Markdown 语法
    .replace(/[#*>`\-\[\]()!_~]/g, "")
    // 移除多余空白
    .replace(/\s+/g, " ");

  return text.length;
}

/**
 * 计算推荐阅读时长（分钟）
 */
export function calculateReadingTime(
  markdown: string,
  difficulty: NoteDifficulty,
  mermaidCount: number,
  screenshotCount: number,
  codeLineCount: number
): number {
  const wordCount = countNoteWords(markdown);
  const baseMinutes = wordCount / 400; // 400 字/分钟
  const chartMinutes =
    (mermaidCount * 15 + screenshotCount * 10 + codeLineCount * 2) / 60;

  const coefficient = {
    beginner: 1.0,
    intermediate: 1.3,
    advanced: 1.6,
  }[difficulty];

  return Math.max(1, Math.round((baseMinutes + chartMinutes) * coefficient));
}

/**
 * 提取标签 — 从笔记 Markdown 中提取 3-6 个标签
 */
export function extractTags(markdown: string): string[] {
  const tagMatch = markdown.match(/🏷️\s*(#[^\s]+(?:\s+#[^\s]+)*)/);
  if (tagMatch) {
    return tagMatch[1].split(/\s+/).filter((t) => t.startsWith("#"));
  }
  return [];
}
