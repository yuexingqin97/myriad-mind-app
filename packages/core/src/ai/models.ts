// ============================================================
// 模型注册表 — DeepSeek V4 Pro / Flash
// ============================================================

import type { ModelInfo } from "./types.js";

export const DEEPSEEK_MODELS: ModelInfo[] = [
  {
    id: "deepseek-v4-pro",
    provider: "deepseek",
    displayName: "DeepSeek V4 Pro",
    aliases: ["deepseek-pro", "v4-pro"],
    contextWindow: 1_000_000,
    maxOutputTokens: 384_000,
    supportsStreaming: true,
    supportsReasoning: true,
    supportsVision: false,
    supportsJsonMode: true,
    costTier: "medium",
    recommendedFor: ["note_generation", "code_analysis", "compare"],
  },
  {
    id: "deepseek-v4-flash",
    provider: "deepseek",
    displayName: "DeepSeek V4 Flash",
    aliases: ["deepseek-flash", "v4-flash"],
    contextWindow: 1_000_000,
    maxOutputTokens: 384_000,
    supportsStreaming: true,
    supportsReasoning: false,
    supportsVision: false,
    supportsJsonMode: true,
    costTier: "low",
    recommendedFor: ["summary", "translation", "next_step_suggestion", "resource_recommend"],
  },
];

/** 按 ID 查找模型 */
export function findModel(id: string): ModelInfo | undefined {
  return DEEPSEEK_MODELS.find((m) => m.id === id || m.aliases.includes(id));
}

/** 按 provider 获取所有模型 */
export function getModelsByProvider(provider: string): ModelInfo[] {
  return DEEPSEEK_MODELS.filter((m) => m.provider === provider);
}
