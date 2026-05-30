import { StyleSheet, Text, View, ScrollView } from "react-native";
import { calculateCultivation, checkAchievements, cultivationEmoji, type NoteStats } from "@myriad-mind/core";

interface Props {
  stats: NoteStats;
}

export function DashboardScreen({ stats }: Props) {
  const cultivation = calculateCultivation(stats);
  const achievements = checkAchievements(stats, []);
  const unlockedCount = achievements.filter((a) => a.unlocked).length;

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.title}>📊 修为面板</Text>

      {/* 境界 */}
      <View style={styles.levelCard}>
        <Text style={styles.levelEmoji}>
          {cultivationEmoji(cultivation.level)}
        </Text>
        <View style={styles.levelInfo}>
          <Text style={styles.levelName}>{cultivation.level}</Text>
          <Text style={styles.levelPoints}>
            {cultivation.points} 修为点
          </Text>
          {cultivation.nextLevel && (
            <Text style={styles.levelNext}>
              距离 {cultivation.nextLevel} 还差{" "}
              {(cultivation.nextLevelPoints ?? 0) - cultivation.points} 点
            </Text>
          )}
        </View>
      </View>

      {/* 进度条 */}
      <View style={styles.progressSection}>
        <View style={styles.progressBar}>
          <View
            style={[styles.progressFill, { width: `${cultivation.progress}%` }]}
          />
        </View>
        <Text style={styles.progressText}>
          {cultivation.progress}% ·{" "}
          {cultivation.level === "渡劫飞升" ? "已达巅峰 💯" : `下一级 ${cultivation.nextLevel}`}
        </Text>
      </View>

      {/* 统计 */}
      <Text style={styles.sectionTitle}>统计</Text>
      <View style={styles.statsGrid}>
        <StatCard icon="📝" label="笔记总数" value={stats.totalNotes.toString()} />
        <StatCard icon="📚" label="知识来源" value={stats.uniqueSources.toString()} />
        <StatCard icon="⏰" label="学习时长" value={`${stats.totalHours}h`} />
        <StatCard icon="🌱" label="入门" value={stats.beginnerNotes.toString()} />
        <StatCard icon="🌿" label="进阶" value={stats.intermediateNotes.toString()} />
        <StatCard icon="🌳" label="深入" value={stats.advancedNotes.toString()} />
        <StatCard icon="🔧" label="技术栈" value={stats.techStacks.toString()} />
        <StatCard icon="📊" label="平均阅读" value={`${stats.avgReadingTime}min`} />
      </View>

      {/* 成就 */}
      <Text style={styles.sectionTitle}>
        🏆 成就 ({unlockedCount}/{achievements.length})
      </Text>
      <View style={styles.achievementsGrid}>
        {achievements.map((a) => (
          <View
            key={a.id}
            style={[
              styles.achievementCard,
              !a.unlocked && styles.achievementLocked,
            ]}
          >
            <Text style={styles.achievementIcon}>
              {a.unlocked ? a.icon : "🔒"}
            </Text>
            <Text
              style={[
                styles.achievementName,
                !a.unlocked && styles.achievementNameLocked,
              ]}
            >
              {a.name}
            </Text>
            <Text style={styles.achievementDesc}>{a.description}</Text>
          </View>
        ))}
      </View>

      {/* 标签分布 */}
      {stats.topTags.length > 0 && (
        <>
          <Text style={styles.sectionTitle}>🏷️ 热门标签</Text>
          <View style={styles.tagsRow}>
            {stats.topTags.slice(0, 5).map((t) => (
              <View key={t.tag} style={styles.tag}>
                <Text style={styles.tagText}>
                  {t.tag} ×{t.count}
                </Text>
              </View>
            ))}
          </View>
        </>
      )}

      <Text style={styles.hint}>
        去「炼化」页面生成更多笔记以提升修为 💪
      </Text>
    </ScrollView>
  );
}

function StatCard({
  icon,
  label,
  value,
}: {
  icon: string;
  label: string;
  value: string;
}) {
  return (
    <View style={styles.statCard}>
      <Text style={styles.statIcon}>{icon}</Text>
      <Text style={styles.statValue}>{value}</Text>
      <Text style={styles.statLabel}>{label}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  content: {
    padding: 20,
    gap: 16,
    paddingBottom: 40,
  },
  title: {
    fontSize: 22,
    fontWeight: "bold",
    color: "#e0e0e0",
  },
  levelCard: {
    flexDirection: "row",
    alignItems: "center",
    gap: 16,
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 16,
    padding: 20,
  },
  levelEmoji: {
    fontSize: 40,
  },
  levelInfo: {
    flex: 1,
  },
  levelName: {
    fontSize: 20,
    fontWeight: "bold",
    color: "#c0a0ff",
  },
  levelPoints: {
    fontSize: 14,
    color: "#e0e0e0",
    marginTop: 4,
  },
  levelNext: {
    fontSize: 11,
    color: "#a0a0c0",
    marginTop: 2,
  },
  progressSection: {
    gap: 6,
  },
  progressBar: {
    height: 6,
    backgroundColor: "#1a1a2e",
    borderRadius: 3,
    overflow: "hidden",
  },
  progressFill: {
    height: "100%",
    backgroundColor: "#6366f1",
    borderRadius: 3,
  },
  progressText: {
    fontSize: 10,
    color: "#666",
  },
  sectionTitle: {
    fontSize: 15,
    fontWeight: "600",
    color: "#c0a0ff",
    marginTop: 8,
  },
  statsGrid: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 10,
  },
  statCard: {
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 12,
    padding: 14,
    alignItems: "center",
    width: "23%",
    minWidth: 75,
    flex: 1,
  },
  statIcon: {
    fontSize: 20,
    marginBottom: 4,
  },
  statValue: {
    fontSize: 18,
    fontWeight: "bold",
    color: "#e0e0e0",
  },
  statLabel: {
    fontSize: 9,
    color: "#777",
    marginTop: 2,
    textAlign: "center",
  },
  achievementsGrid: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 10,
  },
  achievementCard: {
    backgroundColor: "#1a1a2e",
    borderColor: "#6366f1",
    borderWidth: 1,
    borderRadius: 12,
    padding: 14,
    alignItems: "center",
    width: "30%",
    minWidth: 100,
    flex: 1,
  },
  achievementLocked: {
    borderColor: "#2a2a4a",
    opacity: 0.5,
  },
  achievementIcon: {
    fontSize: 24,
    marginBottom: 4,
  },
  achievementName: {
    fontSize: 12,
    fontWeight: "600",
    color: "#e0e0e0",
    textAlign: "center",
  },
  achievementNameLocked: {
    color: "#666",
  },
  achievementDesc: {
    fontSize: 9,
    color: "#777",
    textAlign: "center",
    marginTop: 2,
    lineHeight: 13,
  },
  tagsRow: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 8,
  },
  tag: {
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 8,
    paddingHorizontal: 10,
    paddingVertical: 4,
  },
  tagText: {
    fontSize: 11,
    color: "#a0a0c0",
  },
  hint: {
    textAlign: "center",
    color: "#a0a0c0",
    fontSize: 12,
    marginTop: 12,
  },
});
