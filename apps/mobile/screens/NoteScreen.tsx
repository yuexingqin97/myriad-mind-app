import {
  StyleSheet,
  Text,
  View,
  ScrollView,
  TouchableOpacity,
} from "react-native";
import { WebView } from "react-native-webview";
import { useState } from "react";

// 模拟笔记数据
const mockNotes = [
  {
    title: "Bevy ECS 架构源码分析",
    date: "2026-05-20",
    type: "video",
    difficulty: "advanced" as const,
    summary: "深入分析 Bevy 的 ECS 架构设计，包括 Entity、Component、System 的底层实现以及 Query 机制。",
  },
  {
    title: "Rust 异步编程入门",
    date: "2026-05-18",
    type: "article" as const,
    difficulty: "intermediate" as const,
    summary: "从 Future trait 到 async/await 语法，介绍 Rust 异步编程的核心概念和最佳实践。",
  },
  {
    title: "UE5 蓝图入门教程",
    date: "2026-05-15",
    type: "video" as const,
    difficulty: "beginner" as const,
    summary: "面向零基础的 UE5 可视化编程教程，从蓝图基本概念到实战案例。",
  },
];

const typeIcons: Record<string, string> = {
  video: "🎬",
  article: "📄",
  audio: "🎵",
  code: "💻",
};

const difficultyColors: Record<string, string> = {
  beginner: "#4ade80",
  intermediate: "#facc15",
  advanced: "#f87171",
};

export function NoteScreen() {
  const [selected, setSelected] = useState<number | null>(null);

  if (selected !== null) {
    const note = mockNotes[selected];
    // 移动端用 WebView 渲染 Markdown
    const html = `
<!DOCTYPE html>
<html>
<head>
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <style>
    body {
      font-family: -apple-system, 'PingFang SC', sans-serif;
      padding: 16px;
      background: #0f0f1a;
      color: #e0e0e0;
      line-height: 1.6;
    }
    h1 { color: #e0e0ff; font-size: 20px; }
    h2 { color: #c0c0ff; font-size: 17px; margin-top: 24px; }
    h3 { color: #a0a0ff; font-size: 15px; }
    code { background: #1a1a2e; padding: 2px 6px; border-radius: 4px; }
    pre { background: #1a1a2e; padding: 12px; border-radius: 8px; overflow-x: auto; }
    a { color: #818cf8; }
    img { max-width: 100%; border-radius: 8px; }
    blockquote {
      border-left: 3px solid #6366f1;
      padding-left: 12px;
      color: #a0a0c0;
      margin-left: 0;
    }
  </style>
</head>
<body>
  <h1>${note.title}</h1>
  <p>📅 ${note.date} | ${typeIcons[note.type] ?? "📝"} ${note.type}</p>
  <blockquote>${note.summary}</blockquote>
  <p style="color: #666; margin-top: 24px;">
    ⚠️ 移动端笔记渲染为预览模式。桌面端支持完整 Markdown + Mermaid 图表 + 截图内嵌。
  </p>
</body>
</html>`;

    return (
      <View style={styles.container}>
        <TouchableOpacity
          style={styles.backBtn}
          onPress={() => setSelected(null)}
        >
          <Text style={styles.backBtnText}>← 返回列表</Text>
        </TouchableOpacity>
        <WebView
          source={{ html }}
          style={styles.webview}
          originWhitelist={["*"]}
        />
      </View>
    );
  }

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.title}>📝 学习笔记</Text>
      <Text style={styles.subtitle}>
        共 {mockNotes.length} 篇 · 移动端笔记阅读器
      </Text>

      {mockNotes.map((note, i) => (
        <TouchableOpacity
          key={i}
          style={styles.noteCard}
          onPress={() => setSelected(i)}
        >
          <View style={styles.noteHeader}>
            <Text style={styles.noteIcon}>
              {typeIcons[note.type] ?? "📝"}
            </Text>
            <View style={styles.noteInfo}>
              <Text style={styles.noteTitle} numberOfLines={1}>
                {note.title}
              </Text>
              <Text style={styles.noteDate}>{note.date}</Text>
            </View>
            <View
              style={[
                styles.difficultyBadge,
                { backgroundColor: difficultyColors[note.difficulty] + "22" },
              ]}
            >
              <Text
                style={[
                  styles.difficultyText,
                  { color: difficultyColors[note.difficulty] },
                ]}
              >
                {note.difficulty === "beginner"
                  ? "🌱"
                  : note.difficulty === "intermediate"
                    ? "🌿"
                    : "🌳"}
              </Text>
            </View>
          </View>
          <Text style={styles.noteSummary} numberOfLines={3}>
            {note.summary}
          </Text>
        </TouchableOpacity>
      ))}

      <Text style={styles.emptyHint}>
        去「炼化」页面添加更多笔记吧
      </Text>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  content: {
    padding: 20,
    gap: 12,
  },
  title: {
    fontSize: 22,
    fontWeight: "bold",
    color: "#e0e0e0",
  },
  subtitle: {
    fontSize: 13,
    color: "#a0a0c0",
    marginBottom: 4,
  },
  noteCard: {
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 12,
    padding: 16,
    gap: 8,
  },
  noteHeader: {
    flexDirection: "row",
    alignItems: "center",
    gap: 10,
  },
  noteIcon: {
    fontSize: 20,
  },
  noteInfo: {
    flex: 1,
  },
  noteTitle: {
    fontSize: 15,
    fontWeight: "600",
    color: "#e0e0e0",
  },
  noteDate: {
    fontSize: 11,
    color: "#666",
    marginTop: 2,
  },
  difficultyBadge: {
    paddingHorizontal: 8,
    paddingVertical: 2,
    borderRadius: 10,
  },
  difficultyText: {
    fontSize: 12,
  },
  noteSummary: {
    fontSize: 12,
    color: "#a0a0c0",
    lineHeight: 18,
  },
  emptyHint: {
    textAlign: "center",
    color: "#a0a0c0",
    fontSize: 12,
    marginTop: 20,
  },
  backBtn: {
    padding: 14,
    borderBottomWidth: 1,
    borderBottomColor: "#1e1e3a",
  },
  backBtnText: {
    color: "#818cf8",
    fontSize: 14,
  },
  webview: {
    flex: 1,
    backgroundColor: "#0f0f1a",
  },
});
