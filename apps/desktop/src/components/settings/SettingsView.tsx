import React, { useState, useEffect, useCallback } from "react";
import { type MyriadMindConfig } from "@myriad-mind/core";
import { ConfigWizard, SettingsPage, type KeychainApi, type DepsInfo, type ThemeMode } from "@myriad-mind/ui";
import * as api from "@/api";
import type { DepResult } from "@/api";

import { useTheme } from "@/hooks/useTheme";
import appIcon from "@/assets/icons/myriad-mind-whale-icon-concept.png";

// ---- Props ----

interface SettingsViewProps {
  config: MyriadMindConfig;
  onSave: (c: MyriadMindConfig) => void;
  /** API Key 单独保存后，重新从 config.json 加载同步 state */
  reloadConfig: () => Promise<void>;
  firstLaunch: boolean;
  onFinishWizard: () => void;
  /** 导航到炼化页（完成向导后） */
  onNavigateToInput?: () => void;
}

// ---- Helpers ----

function toDepInfo(d: DepResult | undefined) {
  if (!d) return { name: "未知", found: false };
  return { name: d.name, found: d.found, version: d.version, suggestion: d.suggestion };
}

// ---- Component ----

export function SettingsView({ config, onSave, reloadConfig, firstLaunch, onFinishWizard, onNavigateToInput }: SettingsViewProps) {
  const [showWizard, setShowWizard] = useState(firstLaunch);
  const [deps, setDeps] = useState<DepsInfo | undefined>(undefined);
  const { theme, setTheme } = useTheme();

  // Detect deps
  const detectDeps = useCallback(async () => {
    try {
      const all = await api.detectAllDeps(config.python_path || undefined);
      setDeps({
        python: toDepInfo(all["python"]),
        ffmpeg: toDepInfo(all["ffmpeg"]),
        fasterWhisper: toDepInfo(all["faster-whisper"]),
        ytdlp: toDepInfo(all["yt-dlp"]),
        gpu: toDepInfo(all["gpu"]),
      });
    } catch { /* ignore */ }
  }, [config.python_path]);

  useEffect(() => {
    detectDeps();
  }, [detectDeps]);

  // Keychain adapter
  const keychainAdapter: KeychainApi = React.useMemo(() => {
    // service 名 → config.json 字段名（连字符转下划线：deepseek-api-key → deepseek_api_key）
    const fieldOf = (service: string) => service.replace(/-/g, "_");
    return {
      async check(service: string) {
        const v = await api.getConfigValue(fieldOf(service));
        return !!v && v.trim() !== "";
      },
      async read(service: string) {
        return api.getConfigValue(fieldOf(service));
      },
      async store(service: string, secret: string) {
        await api.setConfigValue(fieldOf(service), secret);
        await reloadConfig(); // 同步 config state，避免后续全量保存覆盖
      },
    };
  }, [reloadConfig]);

  return (
    <div className="view-container">
      <h2 className="view-title">⚙️ 设置</h2>
      <p className="view-subtitle">管理应用配置、API 密钥和功能偏好</p>

      {showWizard ? (
        <ConfigWizard
          config={config}
          onSave={(c, action) => {
            onSave(c);
            if (action === "go_input") {
              onNavigateToInput?.();
            } else {
              onFinishWizard();
            }
          }}
          onCancel={() => { setShowWizard(false); onFinishWizard(); }}
          keychain={keychainAdapter}
          deps={deps}
          onRecheckDeps={detectDeps}
          onSelectOutputDir={async () => api.pickFolder()}
          onOpenOutputDir={() => { /* TBD */ }}
        />
      ) : (
        <>
          <SettingsPage
            config={config}
            onSave={onSave}
            keychain={keychainAdapter}
            deps={deps}
            onRecheckDeps={detectDeps}
            onOpenWizard={() => setShowWizard(true)}
            onSelectOutputDir={async () => api.pickFolder()}
            onOpenOutputDir={() => { /* TBD: open in explorer */ }}
            onOpenCacheDir={() => api.openCacheDir()}
            theme={theme}
            onThemeChange={setTheme}
            appIcon={appIcon}
            onTestAiConnection={async () => api.testDeepSeekConnection()}
          />
        </>
      )}
    </div>
  );
}
