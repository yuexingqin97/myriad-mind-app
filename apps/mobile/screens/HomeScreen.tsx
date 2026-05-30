import { useState } from "react";
import {
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
  ScrollView,
  ActivityIndicator,
} from "react-native";
import { classifyInput, estimateCost, type MyriadMindConfig } from "@myriad-mind/core";

interface Props {
  config: MyriadMindConfig;
}

export function HomeScreen({ config }: Props) {
  const [inputUrl, setInputUrl] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [processing, setProcessing] = useState(false);

  const handleSubmit = async () => {
    if (!inputUrl.trim()) return;

    const classify = classifyInput(inputUrl.trim());
    const estimate = estimateCost(classify, config);

    setStatus(
      `${classify.platform} · 预估 ${estimate.estimatedMinutes} 分钟 · ${Math.round((estimate.inputTokens + estimate.outputTokens) / 1000)}k tokens`
    );
    setProcessing(true);

    // 模拟处理流程（移动端不跑重型脚本，交 Claude API 处理）
    await new Promise((r) => setTimeout(r, 2000));

    setProcessing(false);
    setStatus("✅ 笔记已生成（移动端当前仅支持文章 URL 和本地文本）");
  };

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.title}>📥 神识一扫</Text>
      <Text style={styles.subtitle}>
        万物皆可为笔记。粘贴文章链接或文本，自动炼化为结构化学习笔记。
      </Text>

      {/* 输入框 */}
      <View style={styles.inputGroup}>
        <TextInput
          style={styles.input}
          placeholder="粘贴链接… 如 https://zhuanlan.zhihu.com/p/xxx"
          placeholderTextColor="#666"
          value={inputUrl}
          onChangeText={setInputUrl}
          autoCapitalize="none"
          autoCorrect={false}
        />
        <TouchableOpacity
          style={[
            styles.submitBtn,
            (!inputUrl.trim() || processing) && styles.submitBtnDisabled,
          ]}
          onPress={handleSubmit}
          disabled={!inputUrl.trim() || processing}
        >
          {processing ? (
            <ActivityIndicator color="#e0e0f0" size="small" />
          ) : (
            <Text style={styles.submitBtnText}>炼化</Text>
          )}
        </TouchableOpacity>
      </View>

      {/* 状态 */}
      {status && (
        <View style={styles.statusBar}>
          <Text style={styles.statusText}>{status}</Text>
        </View>
      )}

      {/* 快捷入口 */}
      <View style={styles.quickSection}>
        <QuickEntry icon="🎬" title="在线视频" desc="B 站 / YouTube" />
        <QuickEntry icon="📄" title="在线文章" desc="知乎 / CSDN / 掘金" />
        <QuickEntry icon="📂" title="本地文件" desc="音频 / 文档" />
        <QuickEntry icon="💻" title="代码项目" desc="GitHub 仓库" />
      </View>

      <Text style={styles.note}>
        ⚠️ 移动端不运行重型本地工具（FFmpeg / faster-whisper）。视频处理请使用桌面端。移动端主攻文章笔记和笔记浏览。
      </Text>
    </ScrollView>
  );
}

function QuickEntry({
  icon,
  title,
  desc,
}: {
  icon: string;
  title: string;
  desc: string;
}) {
  return (
    <TouchableOpacity style={styles.quickCard}>
      <Text style={styles.quickIcon}>{icon}</Text>
      <View>
        <Text style={styles.quickTitle}>{title}</Text>
        <Text style={styles.quickDesc}>{desc}</Text>
      </View>
    </TouchableOpacity>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  content: {
    padding: 20,
    gap: 16,
  },
  title: {
    fontSize: 22,
    fontWeight: "bold",
    color: "#e0e0e0",
  },
  subtitle: {
    fontSize: 13,
    color: "#a0a0c0",
    lineHeight: 20,
  },
  inputGroup: {
    gap: 10,
  },
  input: {
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 10,
    padding: 14,
    fontSize: 14,
    color: "#e0e0e0",
  },
  submitBtn: {
    backgroundColor: "#6366f1",
    borderRadius: 10,
    padding: 14,
    alignItems: "center",
  },
  submitBtnDisabled: {
    opacity: 0.5,
  },
  submitBtnText: {
    color: "#e0e0f0",
    fontWeight: "600",
    fontSize: 15,
  },
  statusBar: {
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 10,
    padding: 14,
  },
  statusText: {
    color: "#a0a0c0",
    fontSize: 12,
  },
  quickSection: {
    gap: 10,
  },
  quickCard: {
    flexDirection: "row",
    gap: 12,
    alignItems: "center",
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 10,
    padding: 14,
  },
  quickIcon: {
    fontSize: 24,
  },
  quickTitle: {
    fontSize: 14,
    fontWeight: "600",
    color: "#e0e0e0",
  },
  quickDesc: {
    fontSize: 11,
    color: "#777",
    marginTop: 2,
  },
  note: {
    fontSize: 11,
    color: "#a0a0c0",
    lineHeight: 16,
    marginTop: 8,
  },
});
