// ============================================================
// Prompt 模板 — 英文→中文翻译（含中英对照）
// 与 SKILL.md 步骤 6 对齐
// ============================================================

export interface TranslateContext {
  text: string;
  sourceLang?: string;
  preserveTerms?: string[];
}

/**
 * 构建翻译 Prompt — 英文内容翻译为中文，保留中英对照
 */
export function buildTranslatePrompt(ctx: TranslateContext): string {
  const termsNote =
    ctx.preserveTerms && ctx.preserveTerms.length > 0
      ? [
          `特别注意保留以下技术术语的英文原文：${ctx.preserveTerms.join("、")}`,
          ``,
        ]
      : [];

  return [
    `以下是一段${ctx.sourceLang === "zh" ? "中文" : "英文"}文本。请将其翻译为${
      ctx.sourceLang === "zh" ? "英文" : "中文"
    }，并保留原文对照。`,
    ``,
    `翻译要求：`,
    `1. 准确传达原文含义，不要意译或添加额外内容`,
    `2. 保留技术术语的${
      ctx.sourceLang === "zh" ? "中文" : "英文"
    }原文（括号标注），如：反向传播（backpropagation）`,
    `3. 长句子可适当拆分为短句`,
    `4. 输出格式为：每段先原文，后翻译`,
    ...termsNote,
    `原文：`,
    ctx.text,
  ].join("\n");
}

/**
 * 检测文本中文占比，低于 30% 判定为英文内容需要翻译
 */
export function detectChineseRatio(text: string): number {
  const chineseChars = (text.match(/[一-鿿㐀-䶿]/g) || []).length;
  const totalChars = text.replace(/\s/g, "").length;
  if (totalChars === 0) return 0;
  return chineseChars / totalChars;
}

/**
 * 是否需要翻译：中文占比 < 30%
 */
export function needsTranslation(text: string): boolean {
  return detectChineseRatio(text) < 0.3;
}
