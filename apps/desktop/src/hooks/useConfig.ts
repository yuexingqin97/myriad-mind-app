import { useState, useCallback, useEffect, useRef } from "react";
import { type MyriadMindConfig, type SetupStatus, DEFAULT_CONFIG, safeValidateConfig } from "@myriad-mind/core";
import * as api from "@/api";
import { isTauri } from "@/lib/platform";

// ---- Types ----

interface UseConfigResult {
  config: MyriadMindConfig;
  view: "input" | "dashboard" | "settings";
  setView: (v: "input" | "dashboard" | "settings") => void;
  firstLaunch: boolean;
  /** 配置就绪状态机 — 驱动首页空态 / 向导缺项直达 / 提交守卫 */
  setupStatus: SetupStatus;
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
  const [setupStatus, setSetupStatus] = useState<SetupStatus>("checking");
  const [config, setConfig] = useState<MyriadMindConfig>(DEFAULT_CONFIG);

  // 启动：读 config.json，定初始 setupStatus + 加载 config
  useEffect(() => {
    (async () => {
      if (!(await isTauri())) {
        setSetupStatus("backend_unavailable");
        return;
      }
      const first = await api.isFirstLaunch();
      if (first) {
        setFirstLaunch(true);
        setView("settings");
        setSetupStatus("needs_config");
        return;
      }
      try {
        const raw = await api.readConfig();
        if (!raw || raw === "{}") {
          setSetupStatus("needs_config");
          return;
        }
        const parsed = JSON.parse(raw);
        const result = safeValidateConfig(parsed);
        if (!result.ok) {
          setSetupStatus("invalid_config");
          setConfig((prev) => ({ ...prev, ...parsed })); // 尽量加载可用字段供修复
          return;
        }
        setConfig((prev) => ({ ...prev, ...result.config }));
        // ready / needs_config（DeepSeek Key 判定）交给下面的 config 变化 effect
      } catch {
        setSetupStatus("invalid_config");
      }
    })();
  }, []);

  // 直接修改单个顶层字段（设置页用）— 即时进 App 内存，切换视图不丢
  const update = useCallback(<K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => {
    setConfig((c) => ({ ...c, [key]: value }));
  }, []);

  // config 变化 → 重判 setupStatus（用户配置后实时 ready/needs）。
  // 跳过首次（DEFAULT_CONFIG），避免启动加载前误判。
  const setupInitialized = useRef(false);
  useEffect(() => {
    if (!setupInitialized.current) {
      setupInitialized.current = true;
      return;
    }
    setSetupStatus((prev) => {
      if (prev === "backend_unavailable") return prev;
      const r = safeValidateConfig(config);
      if (!r.ok) return "invalid_config";
      return r.config.deepseek_api_key?.trim() ? "ready" : "needs_config";
    });
  }, [config]);

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

  // 全量保存（配置向导「完成并保存」用）— 立即写盘，不等 debounce。
  // 只管保存，不在此处改 view/firstLaunch —— 导航由调用方（SettingsView 按 action）决定。
  const saveConfig = useCallback((c: MyriadMindConfig) => {
    skipNextAutosave.current = true;
    setConfig(c);
    api.writeConfig(JSON.stringify(c));
  }, []);

  const finishWizard = useCallback(() => {
    setFirstLaunch(false);
  }, []);

  return {
    config,
    view,
    setView,
    firstLaunch,
    setupStatus,
    finishWizard,
    saveConfig,
    update,
  };
}
