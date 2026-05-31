// ============================================================
// 灵力预估 — Token / 成本 / 耗时估算
// 与 SKILL.md 步骤 0.7 对齐
// ============================================================

import type { InputMode, TokenEstimate, MyriadMindConfig } from "./types.js";
import type { ClassifyResult } from "./classifier.js";

// ---- 基准估算表 ----

const BASE_ESTIMATES: Record<
  InputMode,
  {
    inputTokens: number;
    outputTokens: number;
    baseMinutes: number;
    label: string;
  }
> = {
  bilibili: { inputTokens: 40000, outputTokens: 15000, baseMinutes: 6, label: "B 站视频" },
  youtube: { inputTokens: 35000, outputTokens: 12000, baseMinutes: 5, label: "YouTube 视频" },
  douyin: { inputTokens: 8000, outputTokens: 5000, baseMinutes: 2, label: "抖音短视频" },
  xiaohongshu: { inputTokens: 8000, outputTokens: 5000, baseMinutes: 2, label: "小红书视频" },
  article_url: { inputTokens: 15000, outputTokens: 8000, baseMinutes: 2, label: "在线文章" },
  local_video: { inputTokens: 40000, outputTokens: 15000, baseMinutes: 6, label: "本地视频" },
  local_audio: { inputTokens: 25000, outputTokens: 12000, baseMinutes: 4, label: "本地音频" },
  local_text: { inputTokens: 10000, outputTokens: 6000, baseMinutes: 1, label: "本地文档" },
  code_project: { inputTokens: 60000, outputTokens: 25000, baseMinutes: 8, label: "代码项目" },
};

/**
 * 计算灵力预估
 */
export function estimateCost(
  classify: ClassifyResult,
  config: MyriadMindConfig,
  options?: {
    durationMinutes?: number; // 已知视频时长
    fileCount?: number; // 批量文件数
    fileSizeBytes?: number; // 文件大小
  }
): TokenEstimate {
  const base = BASE_ESTIMATES[classify.mode] ?? BASE_ESTIMATES.article_url;

  let { inputTokens, outputTokens, baseMinutes } = base;

  // ---- 时长调整 ----
  if (options?.durationMinutes) {
    const mins = options.durationMinutes;
    if (mins < 10) {
      inputTokens = 20000;
      outputTokens = 10000;
    } else if (mins < 30) {
      inputTokens = 40000;
      outputTokens = 15000;
    } else if (mins < 60) {
      inputTokens = 60000;
      outputTokens = 20000;
    } else {
      inputTokens = 120000;
      outputTokens = 30000;
    }
    baseMinutes = Math.max(2, Math.round(mins * 0.15)); // ASR + 处理约 15% 视频时长
  }

  // ---- 额外消耗 ----
  const factors: string[] = [];

  // ASR 转写
  if (classify.mode !== "article_url" && classify.mode !== "local_text") {
    const asrMinutes = options?.durationMinutes
      ? Math.round(options.durationMinutes * 0.4)
      : 3;
    factors.push(`ASR 转写 (+${asrMinutes} 分钟)`);
    baseMinutes += asrMinutes;
  }

  // 视频下载
  if (classify.needsDownload) {
    factors.push("视频下载 (+2 分钟)");
    baseMinutes += 2;
  }

  // 关键帧
  if (config.features.keyframes && classify.mode !== "article_url" && classify.mode !== "local_text") {
    factors.push("关键帧提取 (+1 分钟)");
    baseMinutes += 1;
    inputTokens += 3000; // 图片 token
  }

  // 评论区
  if (config.features.comments && classify.mode !== "article_url" && classify.mode !== "local_text") {
    factors.push("评论区精华 (+5000 tokens)");
    outputTokens += 5000;
  }

  // Mermaid
  if (config.features.mermaid) {
    outputTokens += Math.round(outputTokens * 0.15); // Mermaid 生成额外 15%
  }

  // 扩展资源
  if (config.features.resources) {
    outputTokens += 2000;
  }

  // 批量文件
  if (options?.fileCount && options.fileCount > 1) {
    const multiplier = Math.min(options.fileCount, 10); // 最多估算 10 倍
    inputTokens *= multiplier;
    outputTokens *= multiplier;
    baseMinutes *= multiplier;
    factors.push(`批量处理 ×${options.fileCount}`);
  }

  // ---- 成本计算 (Claude Sonnet 4.6 定价) ----
  // $3/M 输入 tokens, $15/M 输出 tokens
  const totalCost =
    (inputTokens / 1_000_000) * 3 + (outputTokens / 1_000_000) * 15;

  // ---- 确认级别 ----
  const totalTokens = inputTokens + outputTokens;
  let confirmLevel: "green" | "yellow" | "red";
  if (totalTokens < 30000) {
    confirmLevel = "green";
  } else if (totalTokens < 80000) {
    confirmLevel = "yellow";
  } else {
    confirmLevel = "red";
  }

  return {
    inputTokens,
    outputTokens,
    totalCost: Math.round(totalCost * 10000) / 10000,
    estimatedMinutes: Math.max(1, Math.round(baseMinutes)),
    breakdown: [
      `输入类型：${base.label}`,
      `预估耗时：${Math.max(1, Math.round(baseMinutes))} 分钟`,
      `预估 Token：${Math.round((inputTokens + outputTokens) / 1000)}k (输入 ${Math.round(inputTokens / 1000)}k + 输出 ${Math.round(outputTokens / 1000)}k)`,
      ...(factors.length > 0 ? [`主要消耗：${factors.join("、")}`] : []),
      `预估费用：$${totalCost.toFixed(4)}`,
      `确认级别：${confirmLevel === "green" ? "🟢 直接执行" : confirmLevel === "yellow" ? "🟡 提示后执行" : "🔴 必须确认"}`,
    ].join("\n"),
  };
}

/**
 * 生成用户确认提示文本
 */
export function formatEstimateForUser(estimate: TokenEstimate): string {
  return [
    `🔮 灵力预估`,
    ``,
    estimate.breakdown,
  ].join("\n");
}

/**
 * 快速缩减方案建议
 */
export function suggestReduction(estimate: TokenEstimate): string[] {
  const suggestions: string[] = [];

  if (estimate.inputTokens + estimate.outputTokens > 50000) {
    suggestions.push(
      "**省流模式**：关闭关键帧 + 评论区 → 省 30-40%",
      "**速览模式**：只生成摘要 + 核心概念 + 术语表 → 省 60-70%"
    );
  }

  return suggestions;
}
