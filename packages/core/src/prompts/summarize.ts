// ============================================================
// Prompt 模板 — 视频/文章摘要
// 与 SKILL.md 步骤 5 对齐
// ============================================================

import type { InputMode } from "../types.js";

export interface SummarizeContext {
  title: string;
  platform: string;
  author?: string;
  textContent: string;
  mode: "video" | "article" | "audio";
}

/**
 * 构建视频/文章摘要 Prompt
 */
export function buildSummarizePrompt(ctx: SummarizeContext): string {
  const sourceDesc =
    ctx.mode === "video"
      ? "视频"
      : ctx.mode === "audio"
        ? "音频"
        : "文章";

  return [
    `以下是一个${sourceDesc}的分析素材，请基于这些信息生成总结：`,
    ``,
    `原${sourceDesc}标题：${ctx.title}`,
    ctx.platform ? `来源平台：${ctx.platform}` : null,
    ctx.author ? `作者：${ctx.author}` : null,
    ctx.mode !== "article"
      ? `说明：下面的正文来自平台字幕、自动字幕或语音识别，可能存在少量识别误差、断句问题或专有名词错误。请以原${sourceDesc}标题和上下文为参考，在不改变原意的前提下做适度修正，再完成总结。`
      : null,
    ``,
    `${ctx.mode === "article" ? "正文" : "语音识别文本"}：`,
    ctx.textContent,
    ``,
    `请输出 JSON：`,
    `1. aiGeneratedTitle：简洁概括，不超过30字；可以参考原标题，但不要机械照抄，必要时可根据正文纠正明显错误`,
    `2. summary：提炼主要观点和关键信息，200-300字`,
    `3. keyPoints：输出3-5条结构化要点（字符串数组）`,
  ]
    .filter(Boolean)
    .join("\n");
}

/**
 * 构建纯文本摘要（不含 JSON 输出格式）
 */
export function buildSimpleSummarizePrompt(text: string, maxLength = 300): string {
  return [
    `请用${maxLength}字以内概括以下内容的核心观点：`,
    ``,
    text,
  ].join("\n");
}
