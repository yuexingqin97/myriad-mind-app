// ============================================================
// Prompt 模板 — 代码项目分析
// 与 SKILL.md 代码项目分析模式对齐
// ============================================================

export interface CodeAnalysisContext {
  projectName: string;
  sourceUrl?: string; // GitHub URL 或本地路径
  readme?: string;
  structure: string; // find/scan 输出
  keyFiles: Array<{ path: string; content: string }>;
  focus?: "full" | "core" | "overview";
}

/**
 * 构建代码项目分析 Prompt
 */
export function buildCodeAnalysisPrompt(ctx: CodeAnalysisContext): string {
  const focusDesc =
    ctx.focus === "core"
      ? "只重点分析核心源码目录，跳过测试/示例/文档"
      : ctx.focus === "overview"
        ? "快速概览 — 只读 README + 配置 + 目录结构"
        : "完整分析 — 所有源文件";

  return [
    `请分析以下代码项目，生成结构化分析报告。`,
    `分析范围：${focusDesc}`,
    ``,
    `项目名称：${ctx.projectName}`,
    ctx.sourceUrl ? `来源：${ctx.sourceUrl}` : `来源：本地目录`,
    ``,
    ctx.readme
      ? [
          `README 摘要：`,
          ctx.readme,
          ``,
        ]
      : null,
    `项目结构：`,
    `\`\`\``,
    ctx.structure,
    `\`\`\``,
    ``,
    `关键文件内容：`,
    ...ctx.keyFiles.map(
      (f) => [`--- ${f.path} ---`, f.content, ``].join("\n")
    ),
    ``,
    `请输出：`,
    `1. **项目概览** — 功能、定位、目标用户`,
    `2. **核心模块** — 主要模块及其职责`,
    `3. **架构关系** — Mermaid 架构图（graph TD）+ 模块间依赖说明`,
    `4. **关键流程** — 最重要的 1-3 个程序的执行流程（flowchart/sequenceDiagram）`,
    `5. **技术栈清单** — 语言/框架/库及版本`,
    `6. **代码阅读指南** — 从哪里开始读、按什么顺序、各路径含义`,
    `7. **学习建议** — 如果想深入学习此项目，推荐的前置知识和资源`,
    ``,
    `输出为完整 Markdown 文档。`,
  ]
    .flat()
    .filter(Boolean)
    .join("\n");
}
