// ============================================================
// @myriad-mind/ui — 共享 UI 类型
// 纯 UI 层类型，不依赖平台 API
// ============================================================

// ---- 系统依赖状态 ----

/** 单个依赖检测结果 */
export interface DepInfo {
  name: string;
  found: boolean;
  version?: string;
  suggestion?: string;
}

/** 系统依赖检测结果集合 — 桌面端 DepsPanel 检测后注入 UI 组件 */
export interface DepsInfo {
  python: DepInfo;
  ffmpeg: DepInfo;
  ytdlp: DepInfo;
  gpu: DepInfo;
}

// ---- 配置引导 ----

/** 用户使用意图 — 仅 UI 状态，不入 config.json */
export type SetupIntent = "video" | "local_media" | "article" | "code";

// ---- 配置健康度 ----

export type HealthStatus = "ok" | "warning" | "error" | "unconfigured";

export interface HealthItem {
  label: string;
  status: HealthStatus;
  detail?: string;
}
