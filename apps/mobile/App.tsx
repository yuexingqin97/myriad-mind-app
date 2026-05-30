import { useState } from "react";
import {
  SafeAreaView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from "react-native";
import { StatusBar } from "expo-status-bar";
import { cultivationEmoji, calculateCultivation, computeStats, DEFAULT_CONFIG } from "@myriad-mind/core";
import { HomeScreen } from "./screens/HomeScreen";
import { NoteScreen } from "./screens/NoteScreen";
import { ConfigScreen } from "./screens/ConfigScreen";
import { DashboardScreen } from "./screens/DashboardScreen";

type Tab = "home" | "notes" | "dashboard" | "config";

const tabs: Array<{ key: Tab; icon: string; label: string }> = [
  { key: "home", icon: "📥", label: "炼化" },
  { key: "notes", icon: "📝", label: "笔记" },
  { key: "dashboard", icon: "📊", label: "修为" },
  { key: "config", icon: "⚙️", label: "配置" },
];

// 模拟数据
const mockStats = {
  totalNotes: 3,
  beginnerNotes: 1,
  intermediateNotes: 1,
  advancedNotes: 1,
  uniqueSources: 2,
  techStacks: 3,
  totalHours: 5.2,
  avgReadingTime: 15,
  topTags: [{ tag: "#Rust", count: 2 }, { tag: "#Bevy", count: 2 }],
};

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>("home");
  const [config, setConfig] = useState(DEFAULT_CONFIG);

  return (
    <SafeAreaView style={styles.container}>
      <StatusBar style="light" />

      {/* Header */}
      <View style={styles.header}>
        <Text style={styles.headerTitle}>🧘 大衍决</Text>
        <Text style={styles.headerSubtitle}>Myriad Mind</Text>
      </View>

      {/* Content */}
      <View style={styles.content}>
        {activeTab === "home" && <HomeScreen config={config} />}
        {activeTab === "notes" && <NoteScreen />}
        {activeTab === "dashboard" && <DashboardScreen stats={mockStats} />}
        {activeTab === "config" && (
          <ConfigScreen config={config} onSave={setConfig} />
        )}
      </View>

      {/* Tab Bar */}
      <View style={styles.tabBar}>
        {tabs.map((tab) => (
          <TouchableOpacity
            key={tab.key}
            style={[
              styles.tab,
              activeTab === tab.key && styles.tabActive,
            ]}
            onPress={() => setActiveTab(tab.key)}
          >
            <Text style={styles.tabIcon}>{tab.icon}</Text>
            <Text
              style={[
                styles.tabLabel,
                activeTab === tab.key && styles.tabLabelActive,
              ]}
            >
              {tab.label}
            </Text>
          </TouchableOpacity>
        ))}
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#0f0f1a",
  },
  header: {
    paddingHorizontal: 20,
    paddingTop: 12,
    paddingBottom: 8,
    borderBottomWidth: 1,
    borderBottomColor: "#2a2a4a",
    flexDirection: "row" as const,
    alignItems: "baseline" as const,
    gap: 8,
    backgroundColor: "#1a1a2e",
  },
  headerTitle: {
    fontSize: 20,
    fontWeight: "bold" as const,
    color: "#e0e0f0",
  },
  headerSubtitle: {
    fontSize: 11,
    color: "#666",
    textTransform: "uppercase" as const,
    letterSpacing: 1,
  },
  content: {
    flex: 1,
  },
  tabBar: {
    flexDirection: "row" as const,
    borderTopWidth: 1,
    borderTopColor: "#2a2a4a",
    backgroundColor: "#1a1a2e",
    paddingBottom: 2,
    paddingTop: 2,
  },
  tab: {
    flex: 1,
    alignItems: "center" as const,
    paddingVertical: 8,
    gap: 3,
    borderRadius: 8,
    margin: 2,
    marginHorizontal: 4,
  },
  tabActive: {
    backgroundColor: "rgba(99,102,241,0.15)",
  },
  tabIcon: {
    fontSize: 18,
  },
  tabLabel: {
    fontSize: 10,
    color: "#666",
    fontWeight: "500" as const,
  },
  tabLabelActive: {
    color: "#818cf8",
    fontWeight: "700" as const,
  },
});
