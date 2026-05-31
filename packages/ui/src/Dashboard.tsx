// ============================================================
// Dashboard — 修为面板 (纯 CSS，无 Tailwind 依赖)
// ============================================================

import React from "react";
import type {
  DashboardData,
  CultivationInfo,
  Achievement,
  NoteStats,
  NoteType,
} from "@myriad-mind/core";
import { cultivationEmoji } from "@myriad-mind/core";
import { Card } from "./common/Card.js";

export interface DashboardProps {
  data: DashboardData;
  className?: string;
}

const typeIcons: Record<NoteType, string> = {
  video: "🎬", article: "📄", audio: "🎵", code: "💻", compare: "⚖️",
};

export function Dashboard({ data, className = "" }: DashboardProps) {
  return (
    <div className={className} style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* 境界卡片 */}
      <CultivationDisplay cultivation={data.cultivation} />

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
      <StatsGrid stats={data.stats} />

      {/* 成就 */}
      <AchievementsSection achievements={data.achievements} />

      {/* 最近笔记 */}
      <RecentNotes notes={data.recentNotes} />

      {/* 热门标签 */}
      {data.stats.topTags.length > 0 && (
        <Card title="🏷️ 热门标签" subtitle={`按出现频率排列`}>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
            {data.stats.topTags.map((t, i) => (
              <span key={i} style={{
                padding: "4px 12px", fontSize: 12, borderRadius: 6,
                background: "var(--primary-muted)", border: "1px solid rgba(99,102,241,0.25)",
                color: "var(--primary-hover)",
              }}>
                {t.tag} <span style={{ fontSize: 10, opacity: 0.7 }}>×{t.count}</span>
              </span>
            ))}
          </div>
        </Card>
      )}
    </div>
  );
}

// ---- 境界显示 ----

function CultivationDisplay({ cultivation }: { cultivation: CultivationInfo }) {
  const emoji = cultivationEmoji(cultivation.level);
  const progressLabel =
    cultivation.level === "渡劫飞升"
      ? "💯 已达巅峰"
      : `距离 ${cultivation.nextLevel} 还差 ${(cultivation.nextLevelPoints ?? 0) - cultivation.points} 点`;

  return (
    <Card
      icon={<span style={{ fontSize: 32 }}>{emoji}</span>}
      title={`当前境界：${cultivation.level}`}
      subtitle={`${cultivation.points} 修为点 | ${progressLabel}`}
      variant="elevated"
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11, color: "var(--text-muted)" }}>
          <span>{cultivation.level}</span>
          {cultivation.nextLevel && <span>{cultivation.nextLevel}</span>}
        </div>
        <div style={{
          width: "100%", height: 10, borderRadius: 5,
          background: "var(--bg-root)", border: "1px solid var(--border)",
          overflow: "hidden",
        }}>
          <div style={{
            height: "100%", borderRadius: 5,
            background: "linear-gradient(90deg, #6366f1, #818cf8, #a855f7)",
            boxShadow: "0 0 8px rgba(99,102,241,0.4)",
            transition: "width 0.7s ease",
            width: `${cultivation.progress}%`,
          }} />
        </div>
        <div style={{ textAlign: "center", fontSize: 11, color: "var(--text-muted)" }}>
          {cultivation.progress}% 进度
        </div>
      </div>
    </Card>
  );
}

// ---- 统计面板 ----

function StatsGrid({ stats }: { stats: NoteStats }) {
  const items = [
    { icon: "📝", label: "笔记总数", value: stats.totalNotes },
    { icon: "📚", label: "知识来源", value: stats.uniqueSources },
    { icon: "⏰", label: "学习时长", value: `${stats.totalHours.toFixed(1)}h` },
    { icon: "🌱", label: "入门笔记", value: stats.beginnerNotes },
    { icon: "🌿", label: "进阶笔记", value: stats.intermediateNotes },
    { icon: "🌳", label: "深入笔记", value: stats.advancedNotes },
    { icon: "🔧", label: "技术栈", value: stats.techStacks },
    { icon: "📊", label: "平均阅读", value: `${stats.avgReadingTime}min` },
  ];

  return (
    <div>
      <h3 style={{ fontSize: 13, fontWeight: 600, color: "#c0a0ff", marginBottom: 12, textTransform: "uppercase", letterSpacing: "0.06em" }}>
        📊 统计面板
      </h3>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))", gap: 8 }}>
        {items.map((s) => (
          <StatCard key={s.label} icon={s.icon} label={s.label} value={String(s.value)} />
        ))}
      </div>
    </div>
  );
}

function StatCard({ icon, label, value }: { icon: string; label: string; value: string }) {
  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 12,
      padding: "12px 14px", borderRadius: 12,
      background: "var(--bg-surface)", border: "1px solid var(--border)",
      transition: "border-color 0.15s",
    }}>
      <span style={{ fontSize: 22 }}>{icon}</span>
      <div>
        <p style={{ fontSize: 20, fontWeight: 700, color: "var(--text)", lineHeight: 1, margin: 0 }}>{value}</p>
        <p style={{ fontSize: 10, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em", marginTop: 4, margin: 0 }}>{label}</p>
      </div>
    </div>
  );
}

// ---- 成就 ----

function AchievementsSection({ achievements }: { achievements: Achievement[] }) {
  const unlocked = achievements.filter((a) => a.unlocked);
  const locked = achievements.filter((a) => !a.unlocked);

  return (
    <Card
      title="🏆 成就"
      subtitle={`${unlocked.length}/${achievements.length} 已解锁`}
    >
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(140px, 1fr))", gap: 8 }}>
        {[...unlocked, ...locked].map((a) => (
          <div key={a.id} style={{
            display: "flex", flexDirection: "column", alignItems: "center",
            gap: 6, padding: "14px 10px", borderRadius: 10,
            border: a.unlocked ? "1px solid rgba(99,102,241,0.35)" : "1px solid var(--border)",
            background: a.unlocked ? "rgba(99,102,241,0.08)" : "var(--bg-root)",
            opacity: a.unlocked ? 1 : 0.45,
            textAlign: "center",
          }}>
            <span style={{ fontSize: 24 }}>{a.unlocked ? a.icon : "🔒"}</span>
            <span style={{ fontSize: 12, fontWeight: 600, color: a.unlocked ? "var(--text)" : "var(--text-muted)" }}>
              {a.name}
            </span>
            <span style={{ fontSize: 10, color: "var(--text-muted)", lineHeight: 1.4 }}>
              {a.description}
            </span>
          </div>
        ))}
      </div>
    </Card>
  );
}

// ---- 最近笔记 ----

function RecentNotes({ notes }: { notes: DashboardData["recentNotes"] }) {
  if (notes.length === 0) return null;

  return (
    <Card title="📋 最近笔记" subtitle={`最近 ${notes.length} 篇`}>
      <div style={{ display: "flex", flexDirection: "column" }}>
        {notes.slice(0, 5).map((n, i) => (
          <div key={i} style={{
            display: "flex", alignItems: "center", gap: 12,
            padding: "10px 8px",
            borderBottom: i < notes.slice(0, 5).length - 1 ? "1px solid var(--border)" : "none",
            cursor: "default",
            borderRadius: 6,
          }}>
            <span style={{ fontSize: 18 }}>{typeIcons[n.type] ?? "📝"}</span>
            <div style={{ flex: 1, minWidth: 0 }}>
              <p style={{
                fontSize: 13, fontWeight: 500, color: "var(--text)",
                whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", margin: 0,
              }}>
                {n.title}
              </p>
              <p style={{ fontSize: 11, color: "var(--text-muted)", margin: 0, marginTop: 2 }}>{n.date}</p>
            </div>
          </div>
        ))}
      </div>
    </Card>
  );
}
