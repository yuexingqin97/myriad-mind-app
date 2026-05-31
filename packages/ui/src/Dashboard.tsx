// ============================================================
// Dashboard — 修为面板
// 当前境界 + 核心统计 + 最近笔记 + 知识分布 + 成就 + 建议
// ============================================================

import React, { useState } from "react";
import type {
  DashboardData,
  CultivationInfo,
  Achievement,
  NoteStats,
  NoteType,
} from "@myriad-mind/core";
import { cultivationEmoji } from "@myriad-mind/core";
import { Card } from "./common/Card.js";
import { Button } from "./common/Button.js";

export interface DashboardProps {
  data: DashboardData;
  className?: string;
  /** 打开笔记回调 */
  onOpenNote?: (note: DashboardData["recentNotes"][number]) => void;
  /** 刷新统计回调 */
  onRefresh?: () => void;
  /** 打开输出目录回调 */
  onOpenDir?: () => void;
  /** 空状态"去炼化"回调 */
  onGoToInput?: () => void;
}

const typeIcons: Record<NoteType, string> = {
  video: "🎬", article: "📄", audio: "🎵", code: "💻", compare: "⚖️",
};

// ============================================================

export function Dashboard({
  data,
  className = "",
  onOpenNote,
  onRefresh,
  onOpenDir,
  onGoToInput,
}: DashboardProps) {
  const isEmpty = data.stats.totalNotes === 0;

  return (
    <div className={className} style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* 空状态 */}
      {isEmpty && <EmptyState onGoToInput={onGoToInput} />}

      {/* 操作栏 */}
      {!isEmpty && (
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          {onRefresh && <Button variant="secondary" onClick={onRefresh}>🔄 刷新统计</Button>}
          {onOpenDir && <Button variant="secondary" onClick={onOpenDir}>📂 打开目录</Button>}
        </div>
      )}

      {/* 境界卡片 */}
      {!isEmpty && <CultivationDisplay cultivation={data.cultivation} streak={data.streak} />}

      {/* 连续学习 */}
      {data.streak > 0 && (
        <div style={{
          display: "flex", alignItems: "center", gap: 8,
          padding: "10px 16px", borderRadius: 10,
          background: "rgba(99,102,241,0.1)", border: "1px solid rgba(99,102,241,0.2)",
          fontSize: 13, color: "#a5b4fc",
        }}>
          <span>🔥</span>
          <span style={{ fontWeight: 600 }}>连续学习 {data.streak} 天</span>
        </div>
      )}

      {/* 统计面板 */}
      {!isEmpty && <StatsGrid stats={data.stats} />}

      {/* 最近笔记 + 知识分布 并排 */}
      {!isEmpty && (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, alignItems: "start" }}>
          <RecentNotes notes={data.recentNotes} onOpenNote={onOpenNote} />
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {data.stats.topTags.length > 0 && <TagCloud tags={data.stats.topTags} />}
            <NextSuggestions topTags={data.stats.topTags} />
          </div>
        </div>
      )}

      {/* 成就 */}
      {!isEmpty && <AchievementsSection achievements={data.achievements} />}
    </div>
  );
}

// ============================================================
// Empty State
// ============================================================

function EmptyState({ onGoToInput }: { onGoToInput?: () => void }) {
  return (
    <div style={{ textAlign: "center", padding: "60px 20px" }}>
      <span style={{ fontSize: 56 }}>📝</span>
      <h3 style={{ fontSize: 18, fontWeight: 600, color: "var(--text, #e0e0f0)", margin: "12px 0 8px" }}>
        还没有笔记
      </h3>
      <p style={{ fontSize: 13, color: "var(--text-secondary, #a0a0c0)", margin: 0, lineHeight: 1.6, maxWidth: 400, marginLeft: "auto", marginRight: "auto" }}>
        去炼化页粘贴一个链接，生成的学习笔记会出现在这里。
      </p>
      {onGoToInput && (
        <div style={{ marginTop: 20 }}>
          <Button onClick={onGoToInput}>📥 去炼化</Button>
        </div>
      )}
    </div>
  );
}

// ============================================================
// 境界显示
// ============================================================

function CultivationDisplay({ cultivation, streak }: { cultivation: CultivationInfo; streak: number }) {
  const emoji = cultivationEmoji(cultivation.level);
  const progressLabel =
    cultivation.level === "渡劫飞升"
      ? "💯 已达巅峰"
      : `距离 ${cultivation.nextLevel} 还差 ${(cultivation.nextLevelPoints ?? 0) - cultivation.points} 点`;

  return (
    <Card
      icon={<span style={{ fontSize: 32 }}>{emoji}</span>}
      title={`当前境界：${cultivation.level}`}
      subtitle={`${cultivation.points} 修为点 · ${progressLabel}`}
      variant="elevated"
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11, color: "var(--text-muted, #666)" }}>
          <span>{cultivation.level}</span>
          {cultivation.nextLevel && <span>{cultivation.nextLevel}</span>}
        </div>
        <div style={{
          width: "100%", height: 10, borderRadius: 5,
          background: "var(--bg-app, #0f0f1a)", border: "1px solid var(--border, #2a2a4a)",
          overflow: "hidden",
        }}>
          <div style={{
            height: "100%", borderRadius: 5,
            background: "linear-gradient(90deg, var(--brand-primary), var(--brand-hover), #a855f7)",
            boxShadow: "0 0 8px rgba(99,102,241,0.4)",
            transition: "width 0.7s ease",
            width: `${cultivation.progress}%`,
          }} />
        </div>
        <div style={{ textAlign: "center", fontSize: 11, color: "var(--text-muted, #666)" }}>
          {cultivation.progress}% 进度
        </div>
      </div>
    </Card>
  );
}

// ============================================================
// 统计面板 — 5 核心 + 可展开次要
// ============================================================

function StatsGrid({ stats }: { stats: NoteStats }) {
  const [showMore, setShowMore] = useState(false);

  const coreStats = [
    { icon: "📝", label: "笔记总数", value: stats.totalNotes },
    { icon: "⏰", label: "学习时长", value: `${stats.totalHours.toFixed(1)}h` },
    { icon: "📚", label: "知识来源", value: stats.uniqueSources },
    { icon: "🔧", label: "技术栈", value: stats.techStacks },
    { icon: "🔥", label: "平均阅读", value: `${stats.avgReadingTime}min` },
  ];

  const secondaryStats = [
    { icon: "🌱", label: "入门笔记", value: stats.beginnerNotes },
    { icon: "🌿", label: "进阶笔记", value: stats.intermediateNotes },
    { icon: "🌳", label: "深入笔记", value: stats.advancedNotes },
  ];

  return (
    <div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))", gap: 8 }}>
        {coreStats.map((s) => (
          <StatCard key={s.label} icon={s.icon} label={s.label} value={String(s.value)} />
        ))}
      </div>

      {/* 展开更多 */}
      <button
        onClick={() => setShowMore(!showMore)}
        style={{
          marginTop: 10, background: "none", border: "none",
          color: "var(--text-muted, #666)", fontSize: 11, cursor: "pointer",
          padding: "4px 0",
        }}
      >
        {showMore ? "▾ 收起详情" : "▸ 展开详情（难度分布）"}
      </button>

      {showMore && (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(130px, 1fr))", gap: 8, marginTop: 8 }}>
          {secondaryStats.map((s) => (
            <StatCard key={s.label} icon={s.icon} label={s.label} value={String(s.value)} small />
          ))}
        </div>
      )}
    </div>
  );
}

function StatCard({ icon, label, value, small }: { icon: string; label: string; value: string; small?: boolean }) {
  return (
    <div style={{
      display: "flex", alignItems: "center", gap: small ? 8 : 12,
      padding: small ? "10px 12px" : "12px 14px", borderRadius: small ? 8 : 12,
      background: "var(--bg-surface, #1a1a2e)", border: "1px solid var(--border, #2a2a4a)",
      transition: "border-color 0.15s",
    }}>
      <span style={{ fontSize: small ? 18 : 22 }}>{icon}</span>
      <div>
        <p style={{ fontSize: small ? 16 : 20, fontWeight: 700, color: "var(--text, #e0e0f0)", lineHeight: 1, margin: 0 }}>
          {value}
        </p>
        <p style={{ fontSize: small ? 9 : 10, color: "var(--text-muted, #666)", textTransform: "uppercase", letterSpacing: "0.05em", margin: 0, marginTop: 4 }}>
          {label}
        </p>
      </div>
    </div>
  );
}

// ============================================================
// 最近笔记
// ============================================================

function RecentNotes({
  notes, onOpenNote,
}: {
  notes: DashboardData["recentNotes"];
  onOpenNote?: DashboardProps["onOpenNote"];
}) {
  if (notes.length === 0) return null;

  return (
    <Card title="📋 最近笔记" subtitle={`最近 ${notes.length} 篇`}>
      <div style={{ display: "flex", flexDirection: "column" }}>
        {notes.slice(0, 8).map((n, i) => (
          <div
            key={i}
            onClick={() => onOpenNote?.(n)}
            title={onOpenNote ? "点击打开笔记" : undefined}
            style={{
              display: "flex", alignItems: "center", gap: 12,
              padding: "10px 8px",
              borderBottom: i < notes.slice(0, 8).length - 1 ? "1px solid var(--border, #2a2a4a)" : "none",
              cursor: onOpenNote ? "pointer" : "default",
              borderRadius: 6,
              transition: "background 0.1s",
            }}
            onMouseEnter={(e) => { if (onOpenNote) (e.currentTarget as HTMLElement).style.backgroundColor = "rgba(255,255,255,0.02)"; }}
            onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.backgroundColor = "transparent"; }}
          >
            <span style={{ fontSize: 18 }}>{typeIcons[n.type] ?? "📝"}</span>
            <div style={{ flex: 1, minWidth: 0 }}>
              <p style={{
                fontSize: 13, fontWeight: 500, color: "var(--text, #e0e0f0)",
                whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", margin: 0,
              }}>
                {n.title}
              </p>
              <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: 0, marginTop: 2 }}>
                {n.date}
              </p>
            </div>
          </div>
        ))}
      </div>
    </Card>
  );
}

// ============================================================
// 标签云
// ============================================================

function TagCloud({ tags }: { tags: NoteStats["topTags"] }) {
  return (
    <Card title="🏷️ 知识分布" subtitle="按出现频率排列">
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        {tags.map((t, i) => (
          <span key={i} style={{
            padding: "4px 12px", fontSize: 12, borderRadius: 6,
            background: "var(--brand-soft, rgba(99,102,241,0.15))",
            border: "1px solid rgba(99,102,241,0.25)",
            color: "var(--brand-hover, var(--brand-hover))",
          }}>
            {t.tag} <span style={{ fontSize: 10, opacity: 0.7 }}>×{t.count}</span>
          </span>
        ))}
      </div>
    </Card>
  );
}

// ============================================================
// 下一步建议
// ============================================================

function NextSuggestions({ topTags }: { topTags: NoteStats["topTags"] }) {
  if (topTags.length === 0) return null;

  // 静态规则：基于 topTags 生成推荐
  const suggestions = generateSuggestions(topTags);
  if (suggestions.length === 0) return null;

  return (
    <Card title="💡 下一步建议" subtitle="基于你的学习方向">
      <ul style={{ margin: 0, paddingLeft: 18, fontSize: 12, color: "var(--text-secondary, #a0a0c0)", lineHeight: 1.8 }}>
        {suggestions.map((s, i) => (
          <li key={i}>{s}</li>
        ))}
      </ul>
    </Card>
  );
}

/** 基于标签的简单推荐规则 */
function generateSuggestions(tags: Array<{ tag: string; count: number }>): string[] {
  const tagNames = tags.map((t) => t.tag.toLowerCase());
  const suggestions: string[] = [];

  if (tagNames.some((t) => t.includes("rust") || t.includes("tauri"))) {
    suggestions.push("Tauri 文件系统权限模型深入");
    suggestions.push("Rust async runtime 基础（tokio）");
  }
  if (tagNames.some((t) => t.includes("ai") || t.includes("claude") || t.includes("llm"))) {
    suggestions.push("DeepSeek 1M 上下文利用策略");
    suggestions.push("AI Agent 工具调用模式设计");
  }
  if (tagNames.some((t) => t.includes("react") || t.includes("frontend"))) {
    suggestions.push("React 19 Server Components 迁移指南");
  }
  if (suggestions.length === 0) {
    // 通用推荐
    suggestions.push("尝试处理不同类型的输入内容（文章/视频/代码仓库）");
    suggestions.push("查看热门标签下的笔记，发现知识关联");
  }

  return suggestions.slice(0, 4);
}

// ============================================================
// 成就
// ============================================================

function AchievementsSection({ achievements }: { achievements: Achievement[] }) {
  const unlocked = achievements.filter((a) => a.unlocked);
  const locked = achievements.filter((a) => !a.unlocked);

  return (
    <Card title="🏆 成就" subtitle={`${unlocked.length}/${achievements.length} 已解锁`}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(140px, 1fr))", gap: 8 }}>
        {[...unlocked, ...locked].map((a) => (
          <div key={a.id} style={{
            display: "flex", flexDirection: "column", alignItems: "center",
            gap: 6, padding: "14px 10px", borderRadius: 10,
            border: a.unlocked ? "1px solid rgba(99,102,241,0.35)" : "1px solid var(--border, #2a2a4a)",
            background: a.unlocked ? "rgba(99,102,241,0.08)" : "var(--bg-app, #0f0f1a)",
            opacity: a.unlocked ? 1 : 0.45,
            textAlign: "center",
          }}>
            <span style={{ fontSize: 24 }}>{a.unlocked ? a.icon : "🔒"}</span>
            <span style={{ fontSize: 12, fontWeight: 600, color: a.unlocked ? "var(--text, #e0e0f0)" : "var(--text-muted, #666)" }}>
              {a.name}
            </span>
            <span style={{ fontSize: 10, color: "var(--text-muted, #666)", lineHeight: 1.4 }}>
              {a.description}
            </span>
          </div>
        ))}
      </div>
    </Card>
  );
}
