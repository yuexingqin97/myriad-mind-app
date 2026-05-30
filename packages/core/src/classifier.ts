// ============================================================
// 输入分类器 — 识别用户输入的类型
// 与 SKILL.md 步骤 0 对齐，支持 8 种输入模式
// ============================================================

import type { InputMode } from "./types.js";

export interface ClassifyResult {
  mode: InputMode;
  platform: string; // 具体平台名
  confidence: number; // 0-1
  needsFFmpeg: boolean;
  needsDownload: boolean;
  isLocal: boolean;
  extractedId?: string; // 如 BVxxx / video_id
}

// ---- URL 模式匹配表 ----

const VIDEO_PLATFORMS: Array<{
  pattern: RegExp;
  mode: InputMode;
  name: string;
}> = [
  { pattern: /bilibili\.com|b23\.tv/i, mode: "bilibili", name: "B 站" },
  { pattern: /youtube\.com|youtu\.be/i, mode: "youtube", name: "YouTube" },
  { pattern: /douyin\.com|tiktok\.com/i, mode: "douyin", name: "抖音/TikTok" },
  {
    pattern: /xiaohongshu\.com|xhslink\.com/i,
    mode: "xiaohongshu",
    name: "小红书",
  },
];

const ARTICLE_PLATFORMS: Array<{
  pattern: RegExp;
  name: string;
}> = [
  { pattern: /zhuanlan\.zhihu\.com|zhihu\.com\/question|zhihu\.com\/answer/i, name: "知乎" },
  { pattern: /blog\.csdn\.net|csdn\.net/i, name: "CSDN" },
  { pattern: /juejin\.cn\/post|juejin\.im\/post/i, name: "掘金" },
  { pattern: /jianshu\.com\/p\//i, name: "简书" },
  { pattern: /mp\.weixin\.qq\.com\/s\//i, name: "微信公众号" },
  { pattern: /wikipedia\.org|^https?:\/\/wiki\./i, name: "Wiki" },
];

// ---- 本地文件扩展名 ----

const VIDEO_EXT = new Set([".mp4", ".mov", ".avi", ".mkv", ".webm"]);
const AUDIO_EXT = new Set([".mp3", ".wav", ".m4a", ".flac", ".ogg", ".aac"]);
const TEXT_EXT = new Set([
  ".md",
  ".txt",
  ".pdf",
  ".html",
  ".htm",
  ".rst",
  ".org",
]);
const CODE_EXT = new Set([
  ".py",
  ".js",
  ".ts",
  ".tsx",
  ".jsx",
  ".rs",
  ".go",
  ".java",
  ".c",
  ".cpp",
  ".h",
  ".hpp",
  ".cs",
  ".swift",
  ".kt",
  ".scala",
  ".rb",
  ".php",
  ".sh",
  ".sql",
  ".toml",
  ".yaml",
  ".yml",
  ".json",
]);

/**
 * 判断输入类型
 */
export function classifyInput(input: string): ClassifyResult {
  const trimmed = input.trim();

  // 1. 检查是否为 URL
  if (/^https?:\/\//i.test(trimmed)) {
    return classifyUrl(trimmed);
  }

  // 2. 检查是否为本地路径
  return classifyLocalPath(trimmed);
}

/**
 * URL 分类
 */
function classifyUrl(url: string): ClassifyResult {
  // 检查视频平台
  for (const platform of VIDEO_PLATFORMS) {
    if (platform.pattern.test(url)) {
      const id = extractVideoId(url, platform.mode);
      return {
        mode: platform.mode,
        platform: platform.name,
        confidence: 0.95,
        needsFFmpeg: platform.mode !== "youtube",
        needsDownload: true,
        isLocal: false,
        extractedId: id,
      };
    }
  }

  // 检查文章平台
  for (const platform of ARTICLE_PLATFORMS) {
    if (platform.pattern.test(url)) {
      return {
        mode: "article_url",
        platform: platform.name,
        confidence: 0.9,
        needsFFmpeg: false,
        needsDownload: false,
        isLocal: false,
      };
    }
  }

  // 检查 GitHub URL
  if (/github\.com\/[^/]+\/[^/]+/i.test(url)) {
    const match = url.match(/github\.com\/([^/]+)\/([^/?#]+)/i);
    return {
      mode: "local_text", // GitHub → 代码模式由上层路由
      platform: "GitHub",
      confidence: 0.8,
      needsFFmpeg: false,
      needsDownload: true, // 需要 git clone
      isLocal: false,
      extractedId: match ? `${match[1]}/${match[2]}` : undefined,
    };
  }

  // 通用 URL → 尝试文章模式
  return {
    mode: "article_url",
    platform: "通用",
    confidence: 0.6,
    needsFFmpeg: false,
    needsDownload: false,
    isLocal: false,
  };
}

/**
 * 本地路径分类
 */
function classifyLocalPath(path: string): ClassifyResult {
  // 检测是否为目录
  const dirPattern = /[\/\\]$/;
  if (dirPattern.test(path)) {
    return {
      mode: "local_text", // 目录模式由上层根据文件列表判断
      platform: "本地目录",
      confidence: 0.8,
      needsFFmpeg: false,
      needsDownload: false,
      isLocal: true,
    };
  }

  // 按扩展名判断
  const ext = path.match(/\.\w+$/)?.[0]?.toLowerCase() ?? "";

  if (VIDEO_EXT.has(ext)) {
    return {
      mode: "local_video",
      platform: `本地视频 (.${ext})`,
      confidence: 0.95,
      needsFFmpeg: true,
      needsDownload: false,
      isLocal: true,
      extractedId: extractFilename(path),
    };
  }

  if (AUDIO_EXT.has(ext)) {
    return {
      mode: "local_audio",
      platform: `本地音频 (.${ext})`,
      confidence: 0.95,
      needsFFmpeg: false,
      needsDownload: false,
      isLocal: true,
      extractedId: extractFilename(path),
    };
  }

  if (TEXT_EXT.has(ext)) {
    return {
      mode: "local_text",
      platform: `本地文档 (.${ext})`,
      confidence: 0.9,
      needsFFmpeg: false,
      needsDownload: false,
      isLocal: true,
      extractedId: extractFilename(path),
    };
  }

  // 默认 → 尝试读为文本
  return {
    mode: "local_text",
    platform: "本地文件",
    confidence: 0.5,
    needsFFmpeg: false,
    needsDownload: false,
    isLocal: true,
    extractedId: extractFilename(path),
  };
}

/**
 * 提取视频 ID
 */
function extractVideoId(url: string, mode: InputMode): string | undefined {
  switch (mode) {
    case "bilibili": {
      const match = url.match(/bilibili\.com\/video\/(BV[\w]+)/i);
      if (match) return match[1];
      const b23 = url.match(/b23\.tv\/([\w]+)/i);
      return b23?.[1];
    }
    case "youtube": {
      const match = url.match(
        /(?:youtube\.com\/watch\?v=|youtu\.be\/|youtube\.com\/embed\/)([^&?#\s]+)/i
      );
      return match?.[1];
    }
    case "douyin": {
      const match = url.match(/(?:douyin\.com|v\.douyin\.com)\/(?:video\/)?([^?#/\s]+)/i);
      return match?.[1];
    }
    case "xiaohongshu": {
      const match = url.match(/(?:xiaohongshu\.com|xhslink\.com)\/(?:explore\/|discovery\/item\/)?([^?#/\s]+)/i);
      return match?.[1];
    }
    default:
      return undefined;
  }
}

/**
 * 提取文件名（不含扩展名）
 */
function extractFilename(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  const filename = parts[parts.length - 1] ?? path;
  return filename.replace(/\.[^.]+$/, "");
}

/**
 * 判断目录中是否为代码项目（代码文件占比 > 50%）
 */
export function isCodeProject(files: string[]): boolean {
  if (files.length === 0) return false;
  const codeCount = files.filter((f) => {
    const ext = f.match(/\.\w+$/)?.[0]?.toLowerCase() ?? "";
    return CODE_EXT.has(ext);
  }).length;
  return codeCount / files.length > 0.5;
}
