import { Dashboard } from "@myriad-mind/ui";
import { calculateCultivation, checkAchievements, type DashboardData } from "@myriad-mind/core";

// ---- 真实笔记数据（来源: D:/Project/MyClaude/大衍决残卷/）----

const REAL_NOTES = [
  { title: "Bevy Example 2D 全面学习笔记",               date: "2026-05-20", type: "video" as const,   difficulty: "intermediate" as const, readingTime: 25, tags: ["#Rust", "#Bevy", "#2D", "#渲染"] },
  { title: "Bevy ECS 全面学习笔记",                       date: "2026-05-18", type: "video" as const,   difficulty: "intermediate" as const, readingTime: 28, tags: ["#Rust", "#Bevy", "#ECS", "#源码分析"] },
  { title: "Bevy ECS 全面学习笔记（v2 重制版）",           date: "2026-05-19", type: "video" as const,   difficulty: "intermediate" as const, readingTime: 25, tags: ["#Rust", "#Bevy", "#ECS", "#源码分析"] },
  { title: "CodeWhale — 完整深度分析报告",                 date: "2026-05-28", type: "code" as const,    difficulty: "advanced" as const,     readingTime: 45, tags: ["#Rust", "#Agent", "#DeepSeek", "#TUI", "#MCP", "#Architecture"] },
  { title: "CodeWhale — 代码分析报告（快速概览）",         date: "2026-05-28", type: "code" as const,    difficulty: "intermediate" as const, readingTime: 15, tags: ["#Rust", "#Agent", "#DeepSeek", "#TUI"] },
  { title: "GDC — 漫威蜘蛛侠 AI 开发剖析",                date: "2026-05-22", type: "video" as const,   difficulty: "intermediate" as const, readingTime: 18, tags: ["#C++", "#游戏AI", "#GDC", "#行为树", "#状态机", "#战斗系统"] },
  { title: "GDC23 — 战神诸神黄昏：为 3A 续作构建 UI",      date: "2026-05-23", type: "video" as const,   difficulty: "intermediate" as const, readingTime: 22, tags: ["#C++", "#UnrealEngine", "#UI", "#UX", "#GDC", "#3A游戏开发"] },
  { title: "GDC26 — 在 UE5 中达到并稳定 60 帧",            date: "2026-05-25", type: "video" as const,   difficulty: "advanced" as const,     readingTime: 32, tags: ["#C++", "#UnrealEngine", "#UE5", "#性能优化", "#GDC", "#渲染"] },
  { title: "跨平台开发怎么选？Tauri、Electron、Flutter 对比", date: "2026-05-15", type: "video" as const,   difficulty: "beginner" as const,     readingTime: 4,  tags: ["#Rust", "#JavaScript", "#Dart", "#跨平台", "#Tauri", "#Electron", "#Flutter"] },
  { title: "Unreal Engine State Tree 完全入门",             date: "2026-05-16", type: "video" as const,   difficulty: "beginner" as const,     readingTime: 15, tags: ["#C++", "#UnrealEngine", "#UE5", "#AI", "#StateTree"] },
  { title: "UOD2022 — 从行为树到状态树：UE5 StateTree 深度解析", date: "2026-05-17", type: "video" as const, difficulty: "intermediate" as const, readingTime: 20, tags: ["#C++", "#UnrealEngine", "#UE5", "#AI", "#StateTree", "#游戏AI"] },
  { title: "UE 反射系统 — 从原理到源码",                    date: "2026-05-26", type: "article" as const, difficulty: "advanced" as const,     readingTime: 18, tags: ["#C++", "#UnrealEngine", "#UE5", "#反射", "#源码分析", "#UHT"] },
  { title: "UE5 类型系统源码解读 — 编译与注册流程",         date: "2026-05-27", type: "video" as const,   difficulty: "advanced" as const,     readingTime: 18, tags: ["#C++", "#UnrealEngine", "#UE5", "#反射", "#源码分析"] },
  { title: "SimpleECS — 代码分析报告",                     date: "2026-05-21", type: "code" as const,    difficulty: "intermediate" as const, readingTime: 15, tags: ["#C#", "#ECS", "#Archetype", "#Unity"] },
  { title: "CC Switch — 代码分析报告",                     date: "2026-05-24", type: "code" as const,    difficulty: "advanced" as const,     readingTime: 25, tags: ["#Tauri", "#Rust", "#React", "#TypeScript", "#DesktopApp", "#AI工具", "#架构分析"] },
];

// ---- 计算真实统计 ----

function computeRealStats() {
  const totalNotes = REAL_NOTES.length;
  const beginnerNotes = REAL_NOTES.filter((n) => n.difficulty === "beginner").length;
  const intermediateNotes = REAL_NOTES.filter((n) => n.difficulty === "intermediate").length;
  const advancedNotes = REAL_NOTES.filter((n) => n.difficulty === "advanced").length;

  // 唯一来源（按类型）
  const uniqueSources = new Set(REAL_NOTES.map((n) => n.type)).size;

  // 技术栈（标签去重）
  const allTags = REAL_NOTES.flatMap((n) => n.tags);
  const tagCounts = new Map<string, number>();
  allTags.forEach((t) => tagCounts.set(t, (tagCounts.get(t) ?? 0) + 1));
  const topTags = [...tagCounts.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 12)
    .map(([tag, count]) => ({ tag, count }));

  const totalHours = REAL_NOTES.reduce((sum, n) => sum + n.readingTime, 0) / 60;
  const avgReadingTime = Math.round(REAL_NOTES.reduce((sum, n) => sum + n.readingTime, 0) / totalNotes);
  const techStacks = tagCounts.size;

  return {
    totalNotes, beginnerNotes, intermediateNotes, advancedNotes,
    uniqueSources, techStacks, totalHours, avgReadingTime,
    topTags,
  };
}

const realStats = computeRealStats();

const realDashboardData: DashboardData = {
  cultivation: calculateCultivation(realStats),
  stats: realStats,
  achievements: checkAchievements(realStats, []),
  recentNotes: REAL_NOTES.sort((a, b) => b.date.localeCompare(a.date)).slice(0, 8).map((n) => ({
    title: n.title,
    date: n.date,
    type: n.type,
  })),
  streak: 5,
};

// ---- Component ----

export function DashboardView() {
  const handleOpenNote = (note: { title: string; date: string; type: string }) => {
    console.log("[Dashboard] Open note:", note.title);
    // TBD: open with system editor
  };

  const handleRefresh = () => {
    console.log("[Dashboard] Refresh stats");
  };

  const handleOpenDir = () => {
    console.log("[Dashboard] Open note directory: D:/Project/MyClaude/大衍决残卷/");
  };

  return (
    <div className="view-container">
      <h2 className="view-title">📊 修为面板</h2>
      <p className="view-subtitle">修炼进度 · 笔记资产 · 成就系统</p>
      <Dashboard
        data={realDashboardData}
        onOpenNote={handleOpenNote}
        onRefresh={handleRefresh}
        onOpenDir={handleOpenDir}
      />
    </div>
  );
}
