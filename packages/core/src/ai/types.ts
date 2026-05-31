// ============================================================
// AI 模块类型 — 跨平台共享
// ============================================================

import type { InputMode } from "../types.js";

// ---- Provider ----

export type AiProvider = "deepseek";

export type AiTask =
  | "note_generation"
  | "summary"
  | "translation"
  | "code_analysis"
  | "compare"
  | "resource_recommend"
  | "next_step_suggestion";

// ---- Model Info ----

export interface ModelInfo {
  id: string;
  provider: AiProvider;
  displayName: string;
  aliases: string[];
  contextWindow: number;
  maxOutputTokens: number;
  supportsStreaming: boolean;
  supportsReasoning: boolean;
  supportsVision: boolean;
  supportsJsonMode: boolean;
  costTier: "free" | "low" | "medium" | "high";
  recommendedFor: AiTask[];
}

// ---- Task Profile (estimator 产出, routing 消费) ----

export type TaskComplexity = "small" | "medium" | "large" | "extreme";

export interface TaskProfile {
  inputMode: InputMode;
  estimatedInputTokens: number;
  estimatedOutputTokens: number;
  totalEstimatedTokens: number;
  complexity: TaskComplexity;
  requiresLongContext: boolean;
  requiresUserConfirm: boolean;
  enabledFeatures: string[];
}

// ---- Model Resolution (routing 产出) ----

export interface ModelResolution {
  requested?: string;
  resolved: string;
  provider: AiProvider;
  reason: string;
  fallbackChain: string[];
}

// ---- Mind Stream Events (统一流式事件) ----

export type MindStreamEvent =
  | { type: "start"; task: AiTask; provider: string; model: string }
  | { type: "reasoning_delta"; delta: string }
  | { type: "delta"; delta: string }
  | { type: "usage"; inputTokens?: number; outputTokens?: number; reasoningTokens?: number; totalTokens?: number }
  | { type: "done"; text: string; finishReason?: string }
  | { type: "error"; code: string; message: string; retryable: boolean };

// ---- Request / Response ----

export interface MindRequest {
  task: AiTask;
  messages: Array<{ role: string; content: string }>;
  systemPrompt: string;
  modelOverride?: string;
  stream: boolean;
  maxTokens?: number;
  thinking?: { enabled: boolean; effort: "high" | "max" };
}

export interface MindResponse {
  text: string;
  reasoningText?: string;
  provider: string;
  model: string;
  usage?: { inputTokens: number; outputTokens: number; reasoningTokens?: number; totalTokens: number };
  finishReason?: string;
}

// ---- Config ----

export interface AiConfig {
  defaultProvider: AiProvider;
  defaultModel: string;
  fastModel: string;
  taskModels: Partial<Record<AiTask, string>>;
  deepseek: {
    enabled: boolean;
    apiKeyId: string;
    baseUrl: string;
    defaultModel: string;
    fastModel: string;
    thinking: { enabled: boolean; effort: "high" | "max" };
  };
  generation: {
    stream: boolean;
    maxTokens: number;
  };
}

// ---- Prompt Hooks ----

export interface PromptHooks {
  globalNoteStyle?: string;
}

export interface TaskOverlay {
  taskPromptOverlay?: string;
}
