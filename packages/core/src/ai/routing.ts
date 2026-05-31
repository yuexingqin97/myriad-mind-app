// ============================================================
// 模型路由 — 根据 TaskProfile 选择 Pro / Flash
// ============================================================

import type { AiTask, ModelResolution, TaskProfile } from "./types.js";

/**
 * 解析模型选择
 *
 * 优先级:
 * 1. modelOverride（用户显式指定）
 * 2. 任务→模型映射
 * 3. 输入规模分级
 * 4. 默认 Pro
 */
export function resolveModel(
  task: AiTask,
  profile?: TaskProfile,
  modelOverride?: string,
): ModelResolution {
  const fallbackChain: string[] = [];

  // 用户显式覆盖
  if (modelOverride) {
    return {
      requested: modelOverride,
      resolved: modelOverride,
      provider: "deepseek",
      reason: "用户指定模型",
      fallbackChain,
    };
  }

  // 快速任务 → Flash
  const fastTasks: AiTask[] = ["summary", "translation", "next_step_suggestion", "resource_recommend"];
  if (fastTasks.includes(task)) {
    return {
      resolved: "deepseek-v4-flash",
      provider: "deepseek",
      reason: `${task} 使用快速模型`,
      fallbackChain,
    };
  }

  // 代码分析 → Pro + max thinking（由调用方设置 thinking）
  if (task === "code_analysis") {
    return {
      resolved: "deepseek-v4-pro",
      provider: "deepseek",
      reason: "代码分析需要深度思考",
      fallbackChain,
    };
  }

  // 根据输入规模
  if (profile) {
    if (profile.complexity === "extreme" || profile.totalEstimatedTokens > 500_000) {
      return {
        resolved: "deepseek-v4-pro",
        provider: "deepseek",
        reason: "大输入需要 1M 上下文长上下文模式",
        fallbackChain,
      };
    }
  }

  // 默认 → Pro
  return {
    resolved: "deepseek-v4-pro",
    provider: "deepseek",
    reason: "默认使用主力模型",
    fallbackChain: ["deepseek-v4-flash"],
  };
}
