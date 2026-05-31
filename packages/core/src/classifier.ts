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
 * @param input 用户输入（URL 或路径）
 * @param files 目录文件列表（仅目录输入时使用，用于判断代码占比）
 */
export function classifyInput(input: string, files?: string[]): ClassifyResult {
  const trimmed = input.trim();

  // 1. Git SSH URL (git@github.com:user/repo.git)
  if (/^git@[\w.-]+:[\w.-]+\/[\w.-]+(\.git)?$/i.test(trimmed)) {
    return classifyGitUrl(trimmed);
  }

  // 2. 检查是否为 HTTP(S) URL
  if (/^https?:\/\//i.test(trimmed)) {
    return classifyUrl(trimmed);
  }

  // 3. 检查是否为本地路径
  return classifyLocalPath(trimmed, files);
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

  // 检查 Git 平台 URL
  if (/github\.com\/[^/]+\/[^/]+/i.test(url)) {
    return classifyGitUrl(url);
  }
  if (/gitlab\.com\/[^/]+\/[^/]+/i.test(url) || /gitee\.com\/[^/]+\/[^/]+/i.test(url)) {
    return classifyGitUrl(url);
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
 * Git 仓库 URL 分类（HTTP + SSH）
 */
function classifyGitUrl(url: string): ClassifyResult {
  // 提取 owner/repo
  let match = url.match(/github\.com\/([^/]+)\/([^/?#]+)/i);
  if (!match) match = url.match(/gitlab\.com\/([^/]+)\/([^/?#]+)/i);
  if (!match) match = url.match(/gitee\.com\/([^/]+)\/([^/?#]+)/i);
  if (!match) match = url.match(/git@([\w.]+):([\w.-]+)\/([\w.-]+)(\.git)?$/i);

  let platform = "GitHub";
  if (/gitlab/i.test(url)) platform = "GitLab";
  if (/gitee/i.test(url)) platform = "Gitee";

  const repo = match
    ? (match[2] && match[3] ? `${match[2]}/${match[3]}`.replace(/\.git$/i, "") : `${match[1]}/${match[2]}`.replace(/\.git$/i, ""))
    : undefined;

  return {
    mode: "code_project",
    platform,
    confidence: 0.9,
    needsFFmpeg: false,
    needsDownload: true, // 需要 git clone
    isLocal: false,
    extractedId: repo,
  };
}

/**
 * 本地路径分类
 */
function classifyLocalPath(path: string, files?: string[]): ClassifyResult {
  // 检测是否为目录（路径以分隔符结尾，或无扩展名且路径存在子目录特征）
  const dirPattern = /[\/\\]$/;
  const hasExtension = /\.[a-zA-Z0-9]{1,6}$/.test(path.trimEnd());

  if (dirPattern.test(path) || (!hasExtension && files)) {
    // 有文件列表 → 判断代码占比
    if (files && files.length > 0) {
      const isCode = isCodeProject(files);
      const codeRatio = files.filter((f) => {
        const ext = f.match(/\.\w+$/)?.[0]?.toLowerCase() ?? "";
        return CODE_EXT.has(ext);
      }).length / files.length;

      return {
        mode: isCode ? "code_project" : "local_text",
        platform: isCode
          ? `代码项目（${Math.round(codeRatio * 100)}% 代码文件）`
          : `本地目录（${files.length} 个文件）`,
        confidence: isCode ? 0.85 : 0.8,
        needsFFmpeg: false,
        needsDownload: false,
        isLocal: true,
      };
    }

    // 无文件列表 → 按目录名猜测
    const dirName = path.replace(/[\/\\]+$/, "").split(/[\/\\]/).pop()?.toLowerCase() ?? "";
    const codeHints = ["code", "project", "repo", "src", "engine", "bevy", "rust", "git", "app", "lib", "crate"];
    const isLikelyCode = codeHints.some((hint) => dirName.includes(hint));

    return {
      mode: isLikelyCode ? "code_project" : "local_text",
      platform: isLikelyCode ? "代码项目（推测）" : "本地目录",
      confidence: isLikelyCode ? 0.6 : 0.7,
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

  if (CODE_EXT.has(ext)) {
    return {
      mode: "code_project",
      platform: `代码文件 (.${ext})`,
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
