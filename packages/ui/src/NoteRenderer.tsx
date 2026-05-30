// ============================================================
// NoteRenderer — Markdown 笔记渲染器
// 用 React 渲染 .md 文件，支持 Mermaid 图表 + 语法高亮 + 截图
// ============================================================

import React from "react";
import type { Note, NoteMetadata } from "@myriad-mind/core";
import { cultivationEmoji } from "@myriad-mind/core";

export interface NoteRendererProps {
  note: Note;
  className?: string;
}

/**
 * 完整笔记渲染器 — 按笔记结构渲染
 */
export function NoteRenderer({ note, className = "" }: NoteRendererProps) {
  return (
    <article
      className={[
        "prose max-w-none text-[#e0e0f0]",
        "prose-headings:text-[#e0e0ff] prose-h2:text-[#c0a0ff]",
        "prose-p:text-[#ccc] prose-strong:text-[#e0e0f0]",
        "prose-code:text-indigo-400 prose-code:bg-[#0f0f1a] prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded",
        "prose-a:text-indigo-400 hover:prose-a:text-indigo-300",
        "prose-blockquote:border-indigo-500 prose-blockquote:text-[#a0a0c0]",
        "prose-table:border-[#2a2a4a] prose-th:text-[#c0a0ff] prose-td:border-[#2a2a4a]",
        "prose-li:text-[#ccc]",
        className,
      ].join(" ")}
    >
      {/* 元信息头 */}
      <NoteHeader metadata={note.metadata} />

      {/* 摘要 */}
      {note.sections.summary && (
        <section className="mb-8">
          <h2>一、AI 摘要</h2>
          <div
            className="text-gray-700 dark:text-gray-300"
            dangerouslySetInnerHTML={{
              __html: markdownToHtml(note.sections.summary),
            }}
          />
        </section>
      )}

      {/* 核心概念 */}
      {note.sections.keyConcepts.length > 0 && (
        <section className="mb-8">
          <h2>二、核心概念</h2>
          <ul>
            {note.sections.keyConcepts.map((concept, i) => (
              <li key={i}>{concept}</li>
            ))}
          </ul>
        </section>
      )}

      {/* 详细笔记 */}
      {note.sections.detailedNotes && (
        <section className="mb-8">
          <h2>三、详细笔记</h2>
          <div
            dangerouslySetInnerHTML={{
              __html: markdownToHtml(note.sections.detailedNotes),
            }}
          />
        </section>
      )}

      {/* 术语表 */}
      {note.sections.glossary.length > 0 && (
        <section className="mb-8">
          <h2>四、关键术语表</h2>
          <GlossaryTable terms={note.sections.glossary} />
        </section>
      )}

      {/* 总结 (从 rawMarkdown 提取) */}
      <section className="mb-8">
        <h2>五、总结与思考</h2>
        <div
          className="text-gray-700 dark:text-gray-300"
          dangerouslySetInnerHTML={{
            __html: extractSection(note.rawMarkdown, "总结|思考"),
          }}
        />
      </section>

      {/* 扩展资源 */}
      {note.sections.resources.length > 0 && (
        <section className="mb-8">
          <h2>六、扩展学习资源</h2>
          <ResourceList resources={note.sections.resources} />
        </section>
      )}

      {/* 评论区精华 */}
      {note.sections.commentsDigest.length > 0 && (
        <section className="mb-8">
          <h2>七、评论区精华讨论</h2>
          <CommentsDigest comments={note.sections.commentsDigest} />
        </section>
      )}

      {/* Mermaid 图表 */}
      {note.sections.mermaidDiagrams.length > 0 && (
        <section className="mb-8">
          <h2>知识关系图</h2>
          {note.sections.mermaidDiagrams.map((diagram, i) => (
            <MermaidBlock key={i} code={diagram} />
          ))}
        </section>
      )}
    </article>
  );
}

// ---- 子组件 ----

function NoteHeader({ metadata }: { metadata: NoteMetadata }) {
  return (
    <header className="not-prose mb-8 p-5 rounded-xl bg-[#1a1a2e] border border-[#2a2a4a]">
      <div className="flex flex-wrap items-center gap-2 text-sm text-[#a0a0c0]">
        <span className="font-medium text-[#e0e0f0]">{metadata.source}</span>
        {metadata.sourceUrl && (
          <a href={metadata.sourceUrl} target="_blank" rel="noopener noreferrer" className="text-indigo-400 hover:underline">
            查看原文 ↗
          </a>
        )}
        {metadata.generatedAt && (
          <>
            <span className="mx-1">·</span>
            <span>{new Date(metadata.generatedAt).toLocaleDateString("zh-CN")}</span>
          </>
        )}
        {metadata.duration && (
          <>
            <span className="mx-1">·</span>
            <span>{formatDuration(metadata.duration)}</span>
          </>
        )}
      </div>

      <div className="mt-3 flex flex-wrap gap-2 items-center text-xs">
        <span className="px-2.5 py-1 rounded-full bg-indigo-500/15 text-indigo-300 border border-indigo-500/20">
          📖 {metadata.readingTimeMinutes} 分钟阅读
        </span>
        <DifficultyBadge difficulty={metadata.difficulty} />
        <ReliabilityBadge reliability={metadata.reliability} />
        {metadata.tags.map((tag) => (
          <span key={tag} className="px-2.5 py-1 rounded-full bg-[#0f0f1a] text-[#a0a0c0] border border-[#2a2a4a]">
            {tag}
          </span>
        ))}
      </div>
    </header>
  );
}

function DifficultyBadge({
  difficulty,
}: {
  difficulty: "beginner" | "intermediate" | "advanced";
}) {
  const map = {
    beginner: { icon: "🌱", label: "入门", color: "bg-green-500/15 text-green-400 border-green-500/20" },
    intermediate: { icon: "🌿", label: "进阶", color: "bg-yellow-500/15 text-yellow-400 border-yellow-500/20" },
    advanced: { icon: "🌳", label: "深入", color: "bg-red-500/15 text-red-400 border-red-500/20" },
  };
  const d = map[difficulty];
  return (
    <span className={["px-2.5 py-1 rounded-full border", d.color].join(" ")}>
      {d.icon} {d.label}
    </span>
  );
}

function ReliabilityBadge({ reliability }: { reliability: 0 | 1 | 2 | 3 | 4 | 5 }) {
  const map = {
    5: { icon: "🟢", label: "可信" },
    4: { icon: "🟢", label: "较可信" },
    3: { icon: "🟡", label: "参考" },
    2: { icon: "🟠", label: "谨慎" },
    1: { icon: "🔴", label: "仅作了解" },
    0: { icon: "⚪", label: "未评级" },
  };
  const r = map[reliability] ?? map[3];
  return (
    <span className="px-2.5 py-1 rounded-full bg-[#0f0f1a] text-[#a0a0c0] border border-[#2a2a4a]">
      {r.icon} 可靠性: {r.label}
    </span>
  );
}

function GlossaryTable({
  terms,
}: {
  terms: Array<{ term: string; definition: string; aliases?: string[] }>;
}) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm border-collapse">
        <thead>
          <tr className="border-b border-[#2a2a4a]">
            <th className="text-left py-2 pr-4 font-semibold text-[#c0a0ff]">术语</th>
            <th className="text-left py-2 pr-4 font-semibold text-[#c0a0ff]">释义</th>
          </tr>
        </thead>
        <tbody>
          {terms.map((t, i) => (
            <tr key={i} className="border-b border-[#2a2a4a]">
              <td className="py-2 pr-4 align-top font-medium text-indigo-400">
                {t.term}
                {t.aliases && t.aliases.length > 0 && (
                  <span className="text-xs text-gray-400 ml-1">
                    ({t.aliases.join(", ")})
                  </span>
                )}
              </td>
              <td className="py-2 pr-4 align-top">{t.definition}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ResourceList({
  resources,
}: {
  resources: Array<{
    title: string;
    url: string;
    type: string;
    note?: string;
  }>;
}) {
  const typeIcons: Record<string, string> = {
    paper: "📄",
    tutorial: "📝",
    video: "🎬",
    book: "📚",
    tool: "🔧",
    other: "🔗",
  };

  return (
    <ul className="space-y-2">
      {resources.map((r, i) => (
        <li key={i} className="flex gap-2">
          <span className="shrink-0">{typeIcons[r.type] ?? "🔗"}</span>
          <div>
            <a
              href={r.url}
              target="_blank"
              rel="noopener noreferrer"
              className="text-indigo-500 hover:underline font-medium"
            >
              {r.title}
            </a>
            {r.note && (
              <span className="text-gray-400 text-sm"> — {r.note}</span>
            )}
          </div>
        </li>
      ))}
    </ul>
  );
}

function CommentsDigest({
  comments,
}: {
  comments: Array<{
    author: string;
    content: string;
    likes?: number;
    timestamp?: string;
  }>;
}) {
  return (
    <div className="space-y-4">
      {comments.map((c, i) => (
        <blockquote
          key={i}
          className="border-l-4 border-indigo-500/50 pl-4 py-1 not-italic"
        >
          <div className="flex items-center gap-2 mb-1">
            <span className="font-medium text-sm text-[#e0e0f0]">👤 {c.author}</span>
            {c.likes !== undefined && (
              <span className="text-xs text-[#888]">👍 {c.likes}</span>
            )}
          </div>
          <div className="text-sm text-[#a0a0c0]">
            {c.content}
          </div>
        </blockquote>
      ))}
    </div>
  );
}

/**
 * Mermaid 图表块 — 用 <pre> 包裹源码，客户端可用 mermaid.js 渲染为 SVG
 */
function MermaidBlock({ code }: { code: string }) {
  return (
    <div className="my-4 p-4 rounded-lg bg-[#0f0f1a] border border-[#2a2a4a]">
      <pre className="mermaid text-xs overflow-x-auto text-[#ccc]">{code}</pre>
    </div>
  );
}

// ---- 工具函数 ----

/**
 * 简易 Markdown → HTML（不支持完整 GFM，用于摘要等短文本）
 * 完整渲染推荐在 WebView 中用 marked.js + mermaid
 */
function markdownToHtml(md: string): string {
  return md
    // 粗体
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    // 斜体
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    // 行内代码
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    // 链接
    .replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      '<a href="$2" target="_blank" rel="noopener">$1</a>'
    )
    // 换行
    .replace(/\n\n/g, "</p><p>")
    .replace(/\n/g, "<br/>")
    .replace(/^/, "<p>")
    .replace(/$/, "</p>");
}

/**
 * 从 Markdown 中提取指定标题下的内容
 */
function extractSection(
  markdown: string,
  sectionName: string
): string {
  const regex = new RegExp(
    `## [^\\n]*(${sectionName})[^\\n]*\\n([\\s\\S]*?)(?=\\n## |$)`,
    "i"
  );
  const match = markdown.match(regex);
  return match?.[2]?.trim() ?? "";
}

function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

// ---- 独立渲染辅助 ----

/**
 * 快速渲染纯 Markdown 文本（不含笔记结构）
 * 适用于移动端 WebView 前预处理
 */
export function renderMarkdown(markdown: string): string {
  return markdownToHtml(markdown);
}
