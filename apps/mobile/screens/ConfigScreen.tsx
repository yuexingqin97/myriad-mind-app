import { useState } from "react";
import {
  StyleSheet,
  Text,
  View,
  ScrollView,
  TextInput,
  Switch,
  TouchableOpacity,
} from "react-native";
import type { MyriadMindConfig } from "@myriad-mind/core";

interface Props {
  config: MyriadMindConfig;
  onSave: (config: MyriadMindConfig) => void;
}

export function ConfigScreen({ config, onSave }: Props) {
  const [localConfig, setLocalConfig] = useState<MyriadMindConfig>({
    ...config,
  });

  const updateASR = (field: string, value: unknown) => {
    setLocalConfig((c) => ({
      ...c,
      asr: { ...c.asr, [field]: value },
    }));
  };

  const toggleFeature = (key: keyof MyriadMindConfig["features"]) => {
    setLocalConfig((c) => ({
      ...c,
      features: { ...c.features, [key]: !c.features[key] },
    }));
  };

  const updateOutput = (field: string, value: string | boolean) => {
    setLocalConfig((c) => ({
      ...c,
      output: { ...c.output, [field]: value },
    }));
  };

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.title}>⚙️ 配置</Text>

      {/* ASR */}
      <Section title="语音识别">
        <SettingRow label="ASR 后端">
          <TouchableOpacity
            style={[
              styles.chip,
              localConfig.asr.backend === "faster-whisper" && styles.chipActive,
            ]}
            onPress={() => updateASR("backend", "faster-whisper")}
          >
            <Text
              style={[
                styles.chipText,
                localConfig.asr.backend === "faster-whisper" && styles.chipTextActive,
              ]}
            >
              faster-whisper
            </Text>
          </TouchableOpacity>
          <TouchableOpacity
            style={[
              styles.chip,
              localConfig.asr.backend === "volcengine" && styles.chipActive,
            ]}
            onPress={() => updateASR("backend", "volcengine")}
          >
            <Text
              style={[
                styles.chipText,
                localConfig.asr.backend === "volcengine" && styles.chipTextActive,
              ]}
            >
              火山引擎
            </Text>
          </TouchableOpacity>
        </SettingRow>
      </Section>

      {/* 功能开关 */}
      <Section title="功能开关">
        <FeatureToggle
          label="关键帧截图"
          value={localConfig.features.keyframes}
          onToggle={() => toggleFeature("keyframes")}
        />
        <FeatureToggle
          label="Mermaid 图表"
          value={localConfig.features.mermaid}
          onToggle={() => toggleFeature("mermaid")}
        />
        <FeatureToggle
          label="扩展资源推荐"
          value={localConfig.features.resources}
          onToggle={() => toggleFeature("resources")}
        />
        <FeatureToggle
          label="评论区精华"
          value={localConfig.features.comments}
          onToggle={() => toggleFeature("comments")}
        />
        <FeatureToggle
          label="阅读时长/难度评级"
          value={localConfig.features.reading_info}
          onToggle={() => toggleFeature("reading_info")}
        />
        <FeatureToggle
          label="灵力预估"
          value={localConfig.features.estimation}
          onToggle={() => toggleFeature("estimation")}
        />
      </Section>

      {/* 输出 */}
      <Section title="输出设置">
        <View style={styles.settingRow}>
          <Text style={styles.settingLabel}>笔记输出目录</Text>
          <TextInput
            style={styles.input}
            value={localConfig.output.note_dir}
            placeholder="留空 → 当前目录"
            placeholderTextColor="#666"
            onChangeText={(v) => updateOutput("note_dir", v)}
          />
        </View>
        <FeatureToggle
          label="自动清理临时文件"
          value={localConfig.output.cleanup_temp}
          onToggle={() =>
            updateOutput("cleanup_temp", !localConfig.output.cleanup_temp)
          }
        />
        <FeatureToggle
          label="笔记末尾添加元信息"
          value={localConfig.output.note_metadata}
          onToggle={() =>
            updateOutput("note_metadata", !localConfig.output.note_metadata)
          }
        />
      </Section>

      {/* 保存 */}
      <TouchableOpacity
        style={styles.saveBtn}
        onPress={() => onSave(localConfig)}
      >
        <Text style={styles.saveBtnText}>💾 保存配置</Text>
      </TouchableOpacity>

      <Text style={styles.securityNote}>
        🔒 API Key 存储在 OS 密钥链，永不明文保存。
      </Text>
    </ScrollView>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <View style={styles.section}>
      <Text style={styles.sectionTitle}>{title}</Text>
      {children}
    </View>
  );
}

function SettingRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <View style={styles.settingRow}>
      <Text style={styles.settingLabel}>{label}</Text>
      <View style={styles.settingValue}>{children}</View>
    </View>
  );
}

function FeatureToggle({
  label,
  value,
  onToggle,
}: {
  label: string;
  value: boolean;
  onToggle: () => void;
}) {
  return (
    <View style={styles.settingRow}>
      <Text style={styles.settingLabel}>{label}</Text>
      <Switch
        value={value}
        onValueChange={onToggle}
        trackColor={{ false: "#333", true: "#6366f1" }}
        thumbColor={value ? "#818cf8" : "#666"}
      />
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
  section: {
    backgroundColor: "#1a1a2e",
    borderColor: "#2a2a4a",
    borderWidth: 1,
    borderRadius: 12,
    padding: 16,
    gap: 12,
  },
  sectionTitle: {
    fontSize: 14,
    fontWeight: "600",
    color: "#c0a0ff",
    marginBottom: 4,
  },
  settingRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    paddingVertical: 4,
    gap: 12,
  },
  settingLabel: {
    fontSize: 13,
    color: "#a0a0c0",
    flex: 1,
  },
  settingValue: {
    flexDirection: "row",
    gap: 8,
  },
  chip: {
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 6,
    backgroundColor: "#2a2a4a",
  },
  chipActive: {
    backgroundColor: "#6366f1",
  },
  chipText: {
    fontSize: 12,
    color: "#a0a0c0",
  },
  chipTextActive: {
    color: "#e0e0f0",
  },
  input: {
    backgroundColor: "#2a2a4a",
    borderColor: "#3a3a5a",
    borderWidth: 1,
    borderRadius: 8,
    padding: 8,
    fontSize: 12,
    color: "#e0e0e0",
    flex: 1,
    minWidth: 120,
  },
  saveBtn: {
    backgroundColor: "#6366f1",
    borderRadius: 12,
    padding: 16,
    alignItems: "center",
  },
  saveBtnText: {
    color: "#e0e0f0",
    fontWeight: "600",
    fontSize: 16,
  },
  securityNote: {
    fontSize: 11,
    color: "#555",
    textAlign: "center",
  },
});
