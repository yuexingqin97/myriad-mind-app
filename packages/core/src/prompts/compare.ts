// ============================================================
// Prompt 模板 — 对比模式
// 与 SKILL.md 对比模式对齐
// ============================================================

export interface CompareContext {
  items: Array<{
    title: string;
    source: string;
    type: "video" | "article" | "code" | "note";
    summary: string;
  }>;
}

/**
 * 构建对比分析 Prompt
 */
export function buildComparePrompt(ctx: CompareContext): string {
  const itemsDesc = ctx.items
    .map(
      (item, i) =>
        [
          `**${String.fromCharCode(65 + i)}.** ${item.title}`,
          `   来源：${item.source} | 类型：${item.type}`,
          `   摘要：${item.summary}`,
        ].join("\n")
    )
    .join("\n\n");

  return [
    `请对比分析以下${ctx.items.length}个内容源：`,
    ``,
    itemsDesc,
    ``,
    `请输出对比分析报告：`,
    ``,
    `1. **内容对比表**（Markdown 表格，列为：维度 | ${ctx.items
      .map((_, i) => String.fromCharCode(65 + i))
      .join(" | ")})`,
    `   - 核心观点`,
    `   - 深度/广度`,
    `   - 适合人群`,
    `   - 独特亮点`,
    `   - 不足之处`,
    `2. **观点异同** — 各源同意/互补/冲突的地方`,
    `3. **综合评价** — 哪个在什么方面更优，适合什么场景`,
    `4. **学习建议** — 按什么顺序阅读/观看，如何组合效果最好`,
    ``,
    `输出为完整 Markdown 文档。`,
  ].join("\n");
}

/**
 * 构建简单的两篇笔记对比 Prompt
 */
export function buildSimpleComparePrompt(
  titleA: string,
  summaryA: string,
  titleB: string,
  summaryB: string
): string {
  return [
    `请对比以下两篇内容：`,
    ``,
    `**A:** ${titleA}`,
    summaryA,
    ``,
    `**B:** ${titleB}`,
    summaryB,
    ``,
    `请用表格形式输出对比结果（维度、A、B、优胜方），并给出综合评价。`,
  ].join("\n");
}
