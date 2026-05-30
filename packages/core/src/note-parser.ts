// ============================================================
// 笔记解析器 — 从 .md 文件中提取结构化数据
// ============================================================

import type { Note, NoteMetadata, NoteSections, NoteStats, NoteType, NoteDifficulty } from "./types.js";

// ---- Markdown 块解析 ----

/**
 * 解析 YAML front matter 中的元数据
 */
export function parseFrontMatter(markdown: string): Partial<NoteMetadata> {
  const match = markdown.match(/^---\n([\s\S]*?)\n---/);
  if (!match) return {};

  const front = match[1];
  const meta: Record<string, unknown> = {};

  for (const line of front.split("\n")) {
    const kv = line.match(/^(\w+):\s*(.*)/);
    if (kv) {
      const key = kv[1];
      let value: unknown = kv[2].trim();
      // 简单数组解析 [a, b, c]
      if (typeof value === "string" && value.startsWith("[") && value.endsWith("]")) {
        value = value.slice(1, -1).split(",").map((s) => s.trim().replace(/^["']|["']$/g, ""));
      }
      // 数字解析
      if (typeof value === "string" && /^\d+(\.\d+)?$/.test(value)) {
        value = Number(value);
      }
      meta[key] = value;
    }
  }

  return meta as Partial<NoteMetadata>;
}

/**
 * 按标题拆分章节
 */
export function splitSections(markdown: string): Record<string, string> {
  const sections: Record<string, string> = {};
  const headerRegex = /^## (.+)$/gm;

  const matches: Array<{ title: string; start: number; end: number }> = [];
  let match: RegExpExecArray | null;

  while ((match = headerRegex.exec(markdown)) !== null) {
    if (matches.length > 0) {
      matches[matches.length - 1].end = match.index;
    }
    matches.push({ title: match[1].trim(), start: match.index + match[0].length, end: markdown.length });
  }

  for (const m of matches) {
    sections[m.title] = markdown.slice(m.start, m.end).trim();
  }

  return sections;
}

/**
 * 从 Markdown 中提取术语表（表格格式）
 */
export function extractGlossary(markdown: string): Array<{ term: string; definition: string }> {
  const glossary: Array<{ term: string; definition: string }> = [];
  const tableRegex = /\|(.+)\|(.+)\|/g;
  let inGlossary = false;

  for (const line of markdown.split("\n")) {
    if (/^##\s+.*术语/.test(line) || /^##\s+.*词汇/.test(line)) {
      inGlossary = true;
      continue;
    }
    if (inGlossary && /^##\s+/.test(line)) {
      inGlossary = false;
      continue;
    }
    if (inGlossary) {
      const cells = line.match(/\|([^|]+)\|([^|]+)\|/);
      if (cells && cells[1].trim() !== "---" && cells[1].trim() !== "术语") {
        glossary.push({
          term: cells[1].trim(),
          definition: cells[2].trim(),
        });
      }
    }
  }

  return glossary;
}

// ---- 笔记统计 ----

/**
 * 扫描目录下所有 .md 文件，汇总统计数据
 */
export function computeStats(notes: Note[]): NoteStats {
  const tags = new Map<string, number>();
  let totalHours = 0;

  for (const note of notes) {
    totalHours += (note.metadata.readingTimeMinutes || 0) / 60;

    for (const tag of note.metadata.tags || []) {
      tags.set(tag, (tags.get(tag) ?? 0) + 1);
    }
  }

  const techStackTags = [...tags.entries()]
    .filter(([, count]) => count >= 2)
    .sort((a, b) => b[1] - a[1]);

  const topTags = techStackTags.slice(0, 10).map(([tag, count]) => ({ tag, count }));

  return {
    totalNotes: notes.length,
    beginnerNotes: notes.filter((n) => n.metadata.difficulty === "beginner").length,
    intermediateNotes: notes.filter((n) => n.metadata.difficulty === "intermediate").length,
    advancedNotes: notes.filter((n) => n.metadata.difficulty === "advanced").length,
    uniqueSources: new Set(notes.map((n) => n.metadata.source)).size,
    techStacks: techStackTags.length,
    totalHours: Math.round(totalHours * 10) / 10,
    avgReadingTime:
      notes.length > 0 ? Math.round((notes.reduce((s, n) => s + n.metadata.readingTimeMinutes, 0) / notes.length) * 10) / 10 : 0,
    topTags,
  };
}

/**
 * 试估笔记难度
 */
export function estimateDifficulty(wordCount: number, hasDiagrams: boolean, hasCode: boolean): NoteDifficulty {
  if (hasCode || (hasDiagrams && wordCount > 3000)) return "advanced";
  if (hasDiagrams || wordCount > 1500) return "intermediate";
  return "beginner";
}

/**
 * 试估阅读时间（分钟）
 */
export function estimateReadingTime(wordCount: number): number {
  // 中文阅读速度约 300 字/分钟
  return Math.max(1, Math.round(wordCount / 300));
}
