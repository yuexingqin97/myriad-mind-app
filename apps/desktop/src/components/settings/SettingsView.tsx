import React, { useState, useEffect, useCallback } from "react";
import { type MyriadMindConfig } from "@myriad-mind/core";
import { ConfigWizard, SettingsPage, type KeychainApi, type DepsInfo, type ThemeMode } from "@myriad-mind/ui";
import * as api from "@/api";
import type { DepResult } from "@/api";
import { DepsPanel } from "@/components/settings/DepsPanel";
import { useTheme } from "@/hooks/useTheme";
import appIcon from "@/assets/icons/myriad-mind-whale-icon-concept.png";

// ---- Props ----

interface SettingsViewProps {
  config: MyriadMindConfig;
  onSave: (c: MyriadMindConfig) => void;
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

export function SettingsView({ config, onSave, firstLaunch, onFinishWizard, onNavigateToInput }: SettingsViewProps) {
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
        ytdlp: toDepInfo(all["yt-dlp"]),
        gpu: toDepInfo(all["gpu"]),
      });
    } catch { /* ignore */ }
  }, [config.python_path]);

  useEffect(() => {
    detectDeps();
  }, [detectDeps]);

  // Keychain adapter
  const keychainAdapter: KeychainApi = React.useMemo(() => ({
    async check(service: string) {
      const result = await api.checkKeychainEntry(service, "myriad-mind");
      return result.exists;
    },
    async read(service: string) {
      return api.readKeychainEntry(service, "myriad-mind");
    },
    async store(service: string, secret: string) {
      await api.storeKeychainEntry(service, "myriad-mind", secret);
    },
  }), []);

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
          <DepsPanel pythonPath={config.python_path || undefined} />
          <SettingsPage
            config={config}
            onSave={onSave}
            keychain={keychainAdapter}
            deps={deps}
            onRecheckDeps={detectDeps}
            onOpenWizard={() => setShowWizard(true)}
            onSelectOutputDir={async () => api.pickFolder()}
            onOpenOutputDir={() => { /* TBD: open in explorer */ }}
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
