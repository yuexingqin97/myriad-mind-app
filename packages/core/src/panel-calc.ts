// ============================================================
// 修为面板计算 — 与 docs/架构设计.md §4.3 对齐
// ============================================================

import type {
  NoteStats,
  CultivationLevel,
  CultivationInfo,
  Achievement,
  AchievementId,
  Note,
} from "./types.js";

// ---- 修为等级 ----

const CULTIVATION_LEVELS: Array<{
  level: CultivationLevel;
  minPoints: number;
}> = [
  { level: "炼气期", minPoints: 0 },
  { level: "筑基期", minPoints: 50 },
  { level: "金丹期", minPoints: 120 },
  { level: "元婴期", minPoints: 250 },
  { level: "化神期", minPoints: 500 },
  { level: "大乘期", minPoints: 1000 },
  { level: "渡劫飞升", minPoints: 2000 },
];

/**
 * 计算修为积分
 *
 * 积分规则：
 *   - 每篇笔记 +10
 *   - 中级笔记额外 +5
 *   - 高级笔记额外 +10
 *   - 每个技术栈 +8
 *   - 每小时学习时长 +2
 */
export function calculatePoints(stats: NoteStats): number {
  const points =
    stats.totalNotes * 10 +
    stats.intermediateNotes * 5 +
    stats.advancedNotes * 10 +
    stats.techStacks * 8 +
    Math.floor(stats.totalHours * 2);

  return points;
}

/**
 * 根据积分计算修为境界
 */
export function calculateCultivation(stats: NoteStats): CultivationInfo {
  const points = calculatePoints(stats);

  let currentLevel: CultivationLevel = "炼气期";
  let nextLevel: CultivationLevel | null = null;
  let nextLevelPoints: number | null = null;
  let levelMinPoints = 0;

  for (let i = 0; i < CULTIVATION_LEVELS.length; i++) {
    if (points >= CULTIVATION_LEVELS[i].minPoints) {
      currentLevel = CULTIVATION_LEVELS[i].level;
      levelMinPoints = CULTIVATION_LEVELS[i].minPoints;
      if (i < CULTIVATION_LEVELS.length - 1) {
        nextLevel = CULTIVATION_LEVELS[i + 1].level;
        nextLevelPoints = CULTIVATION_LEVELS[i + 1].minPoints;
      } else {
        nextLevel = null;
        nextLevelPoints = null;
      }
    } else {
      break;
    }
  }

  // 进度百分比
  const progress = nextLevelPoints !== null
    ? Math.round(((points - levelMinPoints) / (nextLevelPoints - levelMinPoints)) * 100)
    : 100;

  return {
    level: currentLevel,
    points,
    nextLevel,
    nextLevelPoints,
    progress: Math.min(100, Math.max(0, progress)),
  };
}

// ---- 成就判定 ----

const ACHIEVEMENTS: Record<
  AchievementId,
  {
    name: string;
    description: string;
    icon: string;
    check: (stats: NoteStats, notes: Note[]) => boolean;
  }
> = {
  "初入道途": {
    name: "初入道途",
    description: "生成第一篇学习笔记",
    icon: "🌱",
    check: (stats) => stats.totalNotes >= 1,
  },
  "博览群书": {
    name: "博览群书",
    description: "累计 50 篇笔记",
    icon: "📚",
    check: (stats) => stats.totalNotes >= 50,
  },
  "专精一道": {
    name: "专精一道",
    description: "某个标签下笔记达到 10 篇",
    icon: "🎯",
    check: (stats) => stats.topTags.some((t) => t.count >= 10),
  },
  "融会贯通": {
    name: "融会贯通",
    description: "高级笔记达到 5 篇",
    icon: "💡",
    check: (stats) => stats.advancedNotes >= 5,
  },
  "持之以恒": {
    name: "持之以恒",
    description: "累计学习时长超过 50 小时",
    icon: "⏰",
    check: (stats) => stats.totalHours >= 50,
  },
  "神识外放": {
    name: "神识外放",
    description: "笔记来源超过 10 个不同平台",
    icon: "👁️",
    check: (stats) => stats.uniqueSources >= 10,
  },
};

/**
 * 检查所有成就的达成情况
 */
export function checkAchievements(stats: NoteStats, notes: Note[]): Achievement[] {
  const results: Achievement[] = [];

  for (const [id, def] of Object.entries(ACHIEVEMENTS) as [
    AchievementId,
    (typeof ACHIEVEMENTS)[AchievementId]
  ][]) {
    const unlocked = def.check(stats, notes);
    results.push({
      id,
      name: def.name,
      description: def.description,
      icon: def.icon,
      unlocked,
      unlockedAt: unlocked
        ? stats.totalNotes > 0
          ? new Date().toISOString()
          : undefined
        : undefined,
    });
  }

  return results;
}

/**
 * 获取境界对应的 emoji
 */
export function cultivationEmoji(level: CultivationLevel): string {
  const map: Record<CultivationLevel, string> = {
    "炼气期": "⚪",
    "筑基期": "🟢",
    "金丹期": "🟡",
    "元婴期": "🟠",
    "化神期": "🔵",
    "大乘期": "🟣",
    "渡劫飞升": "👑",
  };
  return map[level];
}
