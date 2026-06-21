import { useState, useEffect, useCallback, useRef } from "react";
import { type MyriadMindConfig, type SetupStatus } from "@myriad-mind/core";
import { ConfigWizard, SettingsPage, type DepsInfo, type ThemeMode, type WizardInitialStep } from "@myriad-mind/ui";
import * as api from "@/api";
import type { DepResult } from "@/api";

import { useTheme } from "@/hooks/useTheme";
import appIcon from "@/assets/icons/myriad-mind-whale-icon-concept.png";

// ---- Props ----

interface SettingsViewProps {
  config: MyriadMindConfig;
  onSave: (c: MyriadMindConfig) => void;
  /** 设置页字段变更（直接改 App 内存 config，useConfig debounce 自动写盘） */
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  /** 配置就绪状态（驱动向导缺项直达 + 非首启 needs_config 自动开向导） */
  setupStatus: SetupStatus;
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

export function SettingsView({ config, onSave, update, setupStatus, firstLaunch, onFinishWizard, onNavigateToInput }: SettingsViewProps) {
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

  // 缺项直达：缺 DeepSeek Key → keys 步，缺输出目录 → output 步，否则 welcome
  const wizardInitialStep: WizardInitialStep = !config.deepseek_api_key?.trim() ? "keys"
    : !config.output.note_dir?.trim() ? "output" : "welcome";

  // 非首启 + needs_config 时自动开一次向导（直达缺项步）；首启已由 useState(firstLaunch) 自动开
  const autoOpened = useRef(false);
  useEffect(() => {
    if (!autoOpened.current && !firstLaunch && setupStatus === "needs_config") {
      autoOpened.current = true;
      setShowWizard(true);
    }
  }, [setupStatus, firstLaunch]);

  // 重置配置：删除 config.json，下次启动重新进入首启引导
  const handleResetConfig = useCallback(async () => {
    if (!window.confirm("确定重置配置？将删除 ~/.myriad-mind-app/config.json，下次启动重新进入配置向导。")) return;
    try {
      await api.resetConfig();
      window.location.reload();
    } catch (e) {
      window.alert(`重置失败：${e}`);
    }
  }, []);

  // 打开外部链接（注册页等，调系统浏览器）
  const handleOpenUrl = useCallback((url: string) => {
    api.openExternalUrl(url);
  }, []);

  return (
    <div className="view-container">
      <h2 className="view-title">⚙️ 设置</h2>
      <p className="view-subtitle">管理应用配置、API 密钥和功能偏好</p>

      {showWizard ? (
        <ConfigWizard
          config={config}
          initialStep={wizardInitialStep}
          onSave={(c, action) => {
            onSave(c);
            onFinishWizard();      // 结束首启向导态（firstLaunch=false）
            setShowWizard(false);  // 关闭向导，回到 SettingsPage
            if (action === "go_input") {
              onNavigateToInput?.(); // 跳炼化页
            }
            // stay_settings：留在设置页（view 已是 settings），由 setShowWizard(false) 切到 SettingsPage
          }}
          onCancel={() => { setShowWizard(false); onFinishWizard(); }}
          deps={deps}
          onRecheckDeps={detectDeps}
          onSelectOutputDir={async () => api.pickFolder()}
          onOpenOutputDir={(dir) => { if (dir) api.openPath(dir); }}
          onOpenUrl={handleOpenUrl}
        />
      ) : (
        <>
          <SettingsPage
            config={config}
            update={update}
            deps={deps}
            onRecheckDeps={detectDeps}
            onOpenWizard={() => setShowWizard(true)}
            onResetConfig={handleResetConfig}
            onSelectOutputDir={async () => api.pickFolder()}
            onOpenOutputDir={(dir) => { if (dir) api.openPath(dir); }}
            onOpenCacheDir={() => api.openCacheDir()}
            theme={theme}
            onThemeChange={setTheme}
            appIcon={appIcon}
            onTestAiConnection={async () => api.testDeepSeekConnection()}
            onOpenUrl={handleOpenUrl}
          />
        </>
      )}
    </div>
  );
}
