import { useState, useCallback, useEffect } from "react";
import { type MyriadMindConfig, DEFAULT_CONFIG } from "@myriad-mind/core";
import * as api from "@/api";
import { isTauri } from "@/lib/platform";

// ---- Types ----

interface UseConfigResult {
  config: MyriadMindConfig;
  view: "input" | "dashboard" | "settings";
  setView: (v: "input" | "dashboard" | "settings") => void;
  firstLaunch: boolean;
  finishWizard: () => void;
  saveConfig: (c: MyriadMindConfig) => void;
}

// ---- Hook ----

export function useConfig(): UseConfigResult {
  const [view, setView] = useState<"input" | "dashboard" | "settings">("input");
  const [firstLaunch, setFirstLaunch] = useState(false);
  const [config, setConfig] = useState<MyriadMindConfig>(DEFAULT_CONFIG);

  // 启动时加载配置
  useEffect(() => {
    (async () => {
      if (await isTauri()) {
        const first = await api.isFirstLaunch();
        if (first) {
          setFirstLaunch(true);
          setView("settings");
        } else {
          try {
            const raw = await api.readConfig();
            if (raw && raw !== "{}") {
              setConfig((prev) => ({ ...prev, ...JSON.parse(raw) }));
            }
          } catch { /* ignore */ }
        }
      }
    })();
  }, []);

  // 保存设置
  const saveConfig = useCallback((c: MyriadMindConfig) => {
    setConfig(c);
    api.writeConfig(JSON.stringify(c));
    if (firstLaunch) {
      setFirstLaunch(false);
      setView("input");
    }
  }, [firstLaunch]);

  const finishWizard = useCallback(() => {
    setFirstLaunch(false);
  }, []);

  return {
    config,
    view,
    setView,
    firstLaunch,
    finishWizard,
    saveConfig,
  };
}
