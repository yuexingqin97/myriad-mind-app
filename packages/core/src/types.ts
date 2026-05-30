// ============================================================
// 大衍决 core types — 跨平台共享类型定义
// ============================================================

// ---- 输入模式 (8 种) ----
export type InputMode =
  | "bilibili"
  | "youtube"
  | "douyin"
  | "xiaohongshu"
  | "article_url"
  | "local_video"
  | "local_audio"
  | "local_text";

// ---- 管线步骤 ----
export type PipelineStep =
  | "mode_detected"
  | "deps_checked"
  | "estimation"
  | "download_video"
  | "extract_audio"
  | "transcribe_audio"
  | "extract_keyframes"
  | "generate_note"
  | "cleanup"
  | "completed";

export interface PipelineProgress {
  step: PipelineStep;
  label: string;
  percent: number;
  detail?: string;
}

// ---- 灵力预估 ----
export interface TokenEstimate {
  inputTokens: number;
  outputTokens: number;
  totalCost: number; // USD
  estimatedMinutes: number;
  breakdown: string;
}

// ---- 配置类型 (与 Zod Schema 对应) ----
export interface FasterWhisperConfig {
  model_size: "tiny" | "base" | "small" | "medium" | "large-v3";
  device: "auto" | "cpu" | "cuda";
  compute_type?: string;
}

export interface VolcengineConfig {
  token_keyring_id: string;
  appid: string;
}

export interface ASRConfig {
  backend: "faster-whisper" | "volcengine";
  faster_whisper?: FasterWhisperConfig;
  volcengine?: VolcengineConfig;
}

export interface VideoConfig {
  provider: "ai-douyin" | "tikhub";
}

export interface FeaturesConfig {
  keyframes: boolean;
  mermaid: boolean;
  resources: boolean;
  comments: boolean;
  reading_info: boolean;
  estimation: boolean;
}

export interface KeyframesConfig {
  interval: number; // 5-300 秒
  max_frames: number; // 1-200
  mode: "interval" | "scene" | "both";
}

export interface OutputConfig {
  note_dir: string;
  cleanup_temp: boolean;
  note_metadata: boolean;
  debug_metadata: boolean;
}

export interface PostProcessConfig {
  auto_update_panel: boolean;
  auto_suggest_next: boolean;
}

export interface MyriadMindConfig {
  version: 1;
  asr: ASRConfig;
  video: VideoConfig;
  features: FeaturesConfig;
  keyframes: KeyframesConfig;
  output: OutputConfig;
  post_process: PostProcessConfig;
}

// ---- 笔记类型 ----
export type NoteType = "video" | "article" | "audio" | "code" | "compare";

export type NoteDifficulty = "beginner" | "intermediate" | "advanced";

export interface NoteMetadata {
  source: string;
  sourceUrl?: string;
  type: NoteType;
  generatedAt: string; // ISO 8601
  duration?: number; // seconds (video/audio)
  wordCount?: number;
  language?: string;
  difficulty: NoteDifficulty;
  reliability: 0 | 1 | 2 | 3 | 4 | 5;
  readingTimeMinutes: number;
  tags: string[];
  videoId?: string;
}

export interface TermEntry {
  term: string;
  definition: string;
  aliases?: string[];
}

export interface ResourceEntry {
  title: string;
  url: string;
  type: "paper" | "tutorial" | "video" | "book" | "tool" | "other";
  note?: string;
}

export interface CommentEntry {
  author: string;
  content: string;
  likes?: number;
  timestamp?: string;
}

export interface NoteSections {
  summary: string; // 摘要
  keyConcepts: string[]; // 核心概念
  detailedNotes: string; // 详细笔记 (Markdown)
  glossary: TermEntry[]; // 术语表
  resources: ResourceEntry[]; // 扩展资源
  commentsDigest: CommentEntry[]; // 评论区精华
  mermaidDiagrams: string[]; // Mermaid 图表源码
  knowledgeGraph: string; // 知识关系图 (Mermaid)
}

export interface Note {
  metadata: NoteMetadata;
  sections: NoteSections;
  rawMarkdown: string; // 完整 .md 内容
}

// ---- 修为面板 ----
export type CultivationLevel =
  | "炼气期"
  | "筑基期"
  | "金丹期"
  | "元婴期"
  | "化神期"
  | "大乘期"
  | "渡劫飞升";

export interface CultivationInfo {
  level: CultivationLevel;
  points: number;
  nextLevel: CultivationLevel | null;
  nextLevelPoints: number | null;
  progress: number; // 0-100 当前境界进度百分比
}

export type AchievementId =
  | "初入道途"
  | "博览群书"
  | "专精一道"
  | "融会贯通"
  | "持之以恒"
  | "神识外放";

export interface Achievement {
  id: AchievementId;
  name: string;
  description: string;
  icon: string;
  unlocked: boolean;
  unlockedAt?: string; // ISO 8601
}

export interface NoteStats {
  totalNotes: number;
  beginnerNotes: number;
  intermediateNotes: number;
  advancedNotes: number;
  uniqueSources: number;
  techStacks: number; // 涉及技术栈数量
  totalHours: number; // 累计学习时长
  avgReadingTime: number;
  topTags: Array<{ tag: string; count: number }>;
}

export interface DashboardData {
  cultivation: CultivationInfo;
  stats: NoteStats;
  achievements: Achievement[];
  recentNotes: Array<{ title: string; date: string; type: NoteType }>;
  streak: number; // 连续学习天数
}
