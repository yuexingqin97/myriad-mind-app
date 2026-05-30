// ============================================================
// Dashboard — 修为面板 (CC Switch 暗色风格)
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

export function Dashboard({ data, className = "" }: DashboardProps) {
  return (
    <div className={["space-y-5", className].join(" ")}>
      <CultivationDisplay cultivation={data.cultivation} />
      {data.streak > 0 && (
        <div className="flex items-center gap-2 px-4 py-2.5 rounded-lg bg-indigo-500/10 border border-indigo-500/20 text-sm text-indigo-300">
          <span>🔥</span>
          <span className="font-medium">连续学习 {data.streak} 天</span>
        </div>
      )}
      <StatsGrid stats={data.stats} />
      <AchievementsSection achievements={data.achievements} />
      <RecentNotes notes={data.recentNotes} />
    </div>
  );
}

function CultivationDisplay({ cultivation }: { cultivation: CultivationInfo }) {
  const emoji = cultivationEmoji(cultivation.level);
  const progressLabel =
    cultivation.level === "渡劫飞升"
      ? "💯 已达巅峰"
      : `距离 ${cultivation.nextLevel} 还差 ${(cultivation.nextLevelPoints ?? 0) - cultivation.points} 点`;

  return (
    <Card
      icon={<span className="text-3xl">{emoji}</span>}
      title={`当前境界：${cultivation.level}`}
      subtitle={`${cultivation.points} 修为点 | ${progressLabel}`}
      variant="elevated"
    >
      <div className="space-y-2.5">
        <div className="flex justify-between text-xs text-[#666]">
          <span>{cultivation.level}</span>
          {cultivation.nextLevel && <span>{cultivation.nextLevel}</span>}
        </div>
        <div className="w-full h-2.5 rounded-full bg-[#0f0f1a] border border-[#2a2a4a] overflow-hidden">
          <div
            className="h-full rounded-full bg-gradient-to-r from-indigo-500 via-indigo-400 to-purple-500 shadow-[0_0_8px_rgba(99,102,241,0.4)] transition-all duration-700"
            style={{ width: `${cultivation.progress}%` }}
          />
        </div>
        <div className="text-center text-xs text-[#666]">
          {cultivation.progress}% 进度
        </div>
      </div>
    </Card>
  );
}

const typeIcons: Record<NoteType, string> = {
  video: "🎬", article: "📄", audio: "🎵", code: "💻", compare: "⚖️",
};

function StatsGrid({ stats }: { stats: NoteStats }) {
  return (
    <div>
      <h3 className="text-sm font-semibold text-[#c0a0ff] mb-3 uppercase tracking-wide">📊 统计面板</h3>
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-2.5">
        <StatCard icon="📝" label="笔记总数" value={stats.totalNotes.toString()} />
        <StatCard icon="📚" label="知识来源" value={stats.uniqueSources.toString()} />
        <StatCard icon="⏰" label="学习时长" value={`${stats.totalHours.toFixed(1)}h`} />
        <StatCard icon="🌱" label="入门" value={stats.beginnerNotes.toString()} />
        <StatCard icon="🌿" label="进阶" value={stats.intermediateNotes.toString()} />
        <StatCard icon="🌳" label="深入" value={stats.advancedNotes.toString()} />
        <StatCard icon="🔧" label="技术栈" value={stats.techStacks.toString()} />
        <StatCard icon="📊" label="均阅读" value={`${stats.avgReadingTime}min`} />
      </div>
    </div>
  );
}

function StatCard({ icon, label, value }: { icon: string; label: string; value: string }) {
  return (
    <div className="flex items-center gap-3 p-3.5 rounded-xl bg-[#1a1a2e] border border-[#2a2a4a] hover:border-indigo-500/40 transition-colors">
      <span className="text-xl">{icon}</span>
      <div>
        <p className="text-lg font-bold text-[#e0e0f0] leading-none">{value}</p>
        <p className="text-[10px] text-[#a0a0c0] uppercase tracking-wide mt-1">{label}</p>
      </div>
    </div>
  );
}

function AchievementsSection({ achievements }: { achievements: Achievement[] }) {
  const unlocked = achievements.filter((a) => a.unlocked);
  const locked = achievements.filter((a) => !a.unlocked);

  return (
    <Card
      title="🏆 成就"
      subtitle={`${unlocked.length}/${achievements.length} 已解锁`}
    >
      <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
        {[...unlocked, ...locked].map((a) => (
          <div
            key={a.id}
            className={[
              "flex flex-col items-center gap-1.5 p-3.5 rounded-lg border text-center transition-all",
              a.unlocked
                ? "border-indigo-500/40 bg-indigo-500/10"
                : "border-[#2a2a4a] bg-[#0f0f1a] opacity-50",
            ].join(" ")}
          >
            <span className="text-xl">{a.unlocked ? a.icon : "🔒"}</span>
            <span
              className={[
                "text-xs font-semibold",
                a.unlocked ? "text-[#e0e0f0]" : "text-[#666]",
              ].join(" ")}
            >
              {a.name}
            </span>
            <span className="text-[10px] text-[#666] leading-tight">
              {a.description}
            </span>
          </div>
        ))}
      </div>
    </Card>
  );
}

function RecentNotes({ notes }: { notes: DashboardData["recentNotes"] }) {
  if (notes.length === 0) return null;

  return (
    <Card title="📋 最近笔记" subtitle={`最近 ${notes.length} 篇`}>
      <div className="space-y-1">
        {notes.slice(0, 5).map((n, i) => (
          <div
            key={i}
            className="flex items-center gap-3 py-2.5 px-2 -mx-2 rounded-lg border-b border-[#2a2a4a] last:border-0 hover:bg-white/[0.03] transition-colors"
          >
            <span className="text-base">{typeIcons[n.type] ?? "📝"}</span>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-[#e0e0f0] truncate">
                {n.title}
              </p>
              <p className="text-[11px] text-[#666]">{n.date}</p>
            </div>
          </div>
        ))}
      </div>
    </Card>
  );
}
