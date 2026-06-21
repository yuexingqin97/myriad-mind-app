import { useState, useCallback, useEffect, useRef } from "react";
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
  /** 全量保存（配置向导「完成并保存」用）— 立即写盘 */
  saveConfig: (c: MyriadMindConfig) => void;
  /** 直接修改单个顶层字段（设置页用）— 即时进 App 内存 config，由下方 debounce effect 自动写盘 */
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
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

  // 直接修改单个顶层字段（设置页用）— 即时进 App 内存，切换视图不丢
  const update = useCallback(<K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => {
    setConfig((c) => ({ ...c, [key]: value }));
  }, []);

  // debounce 自动写盘：config 变化后 800ms 落盘。
  // 首次 mount 跳过（避免 DEFAULT_CONFIG 回写覆盖磁盘）；saveConfig 已自行写盘时跳过本次。
  const initialMount = useRef(true);
  const skipNextAutosave = useRef(false);
  useEffect(() => {
    if (initialMount.current) {
      initialMount.current = false;
      return;
    }
    if (skipNextAutosave.current) {
      skipNextAutosave.current = false;
      return;
    }
    const timer = window.setTimeout(() => {
      api.writeConfig(JSON.stringify(config));
    }, 800);
    return () => window.clearTimeout(timer);
  }, [config]);

  // 全量保存（配置向导「完成并保存」用）— 立即写盘，不等 debounce
  const saveConfig = useCallback((c: MyriadMindConfig) => {
    skipNextAutosave.current = true; // 已在此处写盘，跳过 effect 的重复写
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
    update,
  };
}
