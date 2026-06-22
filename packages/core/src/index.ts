// ============================================================
// @myriad-mind/core — 公共导出入口
// ============================================================

// Types
export type * from "./types.js";

// Config schema
export {
  ConfigSchema,
  FasterWhisperSchema,
  VolcengineSchema,
  ASRSchema,
  VideoSchema,
  FeaturesSchema,
  KeyframesSchema,
  OutputSchema,
  PostProcessSchema,
  DEFAULT_CONFIG,
  validateConfig,
  safeValidateConfig,
} from "./schema.js";

// Note parser
export {
  parseFrontMatter,
  splitSections,
  extractGlossary,
  computeStats,
  estimateDifficulty,
  estimateReadingTime,
} from "./note-parser.js";

// Panel calculator
export {
  calculatePoints,
  calculateCultivation,
  checkAchievements,
  cultivationEmoji,
} from "./panel-calc.js";

// Input classifier
export {
  classifyInput,
  isCodeProject,
} from "./classifier.js";
export type { ClassifyResult } from "./classifier.js";

// Cost estimator
export {
  estimateCost,
  formatEstimateForUser,
  suggestReduction,
} from "./estimator.js";

// AI module
export {
  DEEPSEEK_MODELS,
  findModel,
  getModelsByProvider,
  resolveModel,
} from "./ai/index.js";
export type {
  AiProvider,
  AiTask,
  ModelInfo,
  TaskComplexity,
  TaskProfile,
  ModelResolution,
  MindStreamEvent,
  MindRequest,
  MindResponse,
  AiConfig,
  PromptHooks,
  TaskOverlay,
} from "./ai/index.js";
