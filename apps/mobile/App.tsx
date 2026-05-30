import { StatusBar } from "expo-status-bar";
import { StyleSheet, Text, View } from "react-native";
import { UI_VERSION } from "@myriad-mind/ui";
import { DEFAULT_CONFIG, cultivationEmoji } from "@myriad-mind/core";

export default function App() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>🧘 大衍决</Text>
      <Text style={styles.tagline}>神识一扫，万物皆可为笔记</Text>
      <View style={styles.card}>
        <Text style={styles.sectionTitle}>修为面板</Text>
        <Text style={styles.level}>
          {cultivationEmoji("炼气期")} 炼气期
        </Text>
        <Text style={styles.hint}>
          万事开头难，先炼化第一篇笔记吧！
        </Text>
        <Text style={styles.meta}>
          UI v{UI_VERSION} · 输出目录:{" "}
          {DEFAULT_CONFIG.output.note_dir || "(未设置)"}
        </Text>
      </View>
      <StatusBar style="light" />
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#0f0f1a",
    alignItems: "center",
    justifyContent: "center",
    padding: 20,
  },
  title: {
    fontSize: 28,
    fontWeight: "bold",
    color: "#e0e0e0",
    marginBottom: 8,
  },
  tagline: {
    fontSize: 14,
    color: "#888",
    marginBottom: 32,
  },
  card: {
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 12,
    padding: 24,
    alignItems: "center",
    width: "100%",
    maxWidth: 320,
  },
  sectionTitle: {
    fontSize: 16,
    fontWeight: "600",
    color: "#c0a0ff",
    marginBottom: 12,
  },
  level: {
    fontSize: 18,
    color: "#e0e0e0",
    marginBottom: 8,
  },
  hint: {
    fontSize: 13,
    color: "#888",
    textAlign: "center",
    lineHeight: 20,
    marginBottom: 16,
  },
  meta: {
    fontSize: 11,
    color: "#555",
  },
});
