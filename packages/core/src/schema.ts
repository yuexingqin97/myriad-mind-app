// ============================================================
// 配置 Zod Schema — 与 docs/architecture.md §4.1 对齐
// ============================================================

import { z } from "zod";

export const FasterWhisperSchema = z.object({
  model_size: z.enum(["tiny", "base", "small", "medium", "large-v3"]),
  device: z.enum(["auto", "cpu", "cuda"]),
  compute_type: z.string().optional(),
});

export const VolcengineSchema = z.object({
  token_keyring_id: z.string(),
  appid: z.string(),
});

export const ASRSchema = z.object({
  backend: z.enum(["faster-whisper", "volcengine"]),
  faster_whisper: FasterWhisperSchema.optional(),
  volcengine: VolcengineSchema.optional(),
});

export const VideoSchema = z.object({
  provider: z.enum(["ai-douyin", "tikhub"]),
});

export const FeaturesSchema = z.object({
  keyframes: z.boolean(),
  mermaid: z.boolean(),
  resources: z.boolean(),
  comments: z.boolean(),
  reading_info: z.boolean(),
  estimation: z.boolean(),
});

export const KeyframesSchema = z.object({
  interval: z.number().min(5).max(300).default(30),
  max_frames: z.number().min(1).max(200).default(50),
  mode: z.enum(["interval", "scene", "both"]).default("interval"),
});

export const OutputSchema = z.object({
  note_dir: z.string().default(""),
  cleanup_temp: z.boolean().default(true),
  note_metadata: z.boolean().default(true),
  debug_metadata: z.boolean().default(false),
});

export const PostProcessSchema = z.object({
  auto_update_panel: z.boolean().default(true),
  auto_suggest_next: z.boolean().default(true),
});

export const ConfigSchema = z.object({
  version: z.literal(1),
  asr: ASRSchema,
  video: VideoSchema,
  features: FeaturesSchema,
  keyframes: KeyframesSchema,
  output: OutputSchema,
  post_process: PostProcessSchema,
});

export type MyriadMindConfig = z.infer<typeof ConfigSchema>;

/**
 * 默认配置 — 开箱即用的安全默认值
 */
export const DEFAULT_CONFIG: MyriadMindConfig = {
  version: 1,
  asr: {
    backend: "faster-whisper",
    faster_whisper: {
      model_size: "medium",
      device: "auto",
    },
  },
  video: {
    provider: "ai-douyin",
  },
  features: {
    keyframes: true,
    mermaid: true,
    resources: true,
    comments: true,
    reading_info: true,
    estimation: true,
  },
  keyframes: {
    interval: 30,
    max_frames: 50,
    mode: "interval",
  },
  output: {
    note_dir: "",
    cleanup_temp: true,
    note_metadata: true,
    debug_metadata: false,
  },
  post_process: {
    auto_update_panel: true,
    auto_suggest_next: true,
  },
};

/**
 * 验证并解析配置，失败时抛出 ZodError
 */
export function validateConfig(raw: unknown): MyriadMindConfig {
  return ConfigSchema.parse(raw);
}

/**
 * 安全验证配置，返回解析成功或错误信息
 */
export function safeValidateConfig(
  raw: unknown
): { ok: true; config: MyriadMindConfig } | { ok: false; error: string } {
  const result = ConfigSchema.safeParse(raw);
  if (result.success) {
    return { ok: true, config: result.data };
  }
  return { ok: false, error: result.error.message };
}
