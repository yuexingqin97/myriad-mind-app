// ============================================================
// Prompt 模板 — 集中导出
// ============================================================

export {
  buildSummarizePrompt,
  buildSimpleSummarizePrompt,
} from "./summarize.js";
export type { SummarizeContext } from "./summarize.js";

export {
  buildTranslatePrompt,
  detectChineseRatio,
  needsTranslation,
} from "./translate.js";
export type { TranslateContext } from "./translate.js";

export {
  buildVideoNotePrompt,
  buildArticleNotePrompt,
  calculateReadingTime,
  countNoteWords,
  extractTags,
} from "./note-gen.js";
export type { NoteGenContext } from "./note-gen.js";

export {
  buildCodeAnalysisPrompt,
} from "./code-analysis.js";
export type { CodeAnalysisContext } from "./code-analysis.js";

export {
  buildComparePrompt,
  buildSimpleComparePrompt,
} from "./compare.js";
export type { CompareContext } from "./compare.js";
