import { useState, useEffect, useCallback } from "react";
import { setLogLevel as invokeSetLogLevel, type LogLevel } from "@/api";

// ============================================================
// useLogLevel — 日志级别运行时偏好（与 useTheme 同款语义）
// ============================================================
//
// 设计要点（与 theme 一致）：
// - 不进 config.json：日志级别是"运行时调试偏好"，不属于业务配置，避免 Schema/Rust 迁移。
// - localStorage 持久化：跨重启保留用户选择。
// - mount 时下发：读 localStorage → 调 api.setLogLevel → Rust log::set_max_level 即时生效。
// - setLogLevel 同时写 localStorage + 下发命令，保证 UI 与 Rust 全局级别同步。
//
// 与 tauri-plugin-log 的关系：Rust 侧 build_log_plugin 默认级别为
// cfg!(debug_assertions) ? Trace : Info，本 hook 在 mount 时覆盖为用户偏好。

const STORAGE_KEY = "myriad-mind-log-level";

/** 默认级别：开发构建偏 trace（便于排查），生产偏 info（用户级，避免刷屏） */
const DEFAULT_LEVEL: LogLevel = import.meta.env.DEV ? "trace" : "info";

function readStoredLevel(): LogLevel {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "trace" || stored === "debug" || stored === "info" || stored === "warn") {
      return stored;
    }
  } catch { /* ignore */ }
  return DEFAULT_LEVEL;
}

export function useLogLevel() {
  const [level, setLevel] = useState<LogLevel>(readStoredLevel);

  // mount 时把用户偏好下发到 Rust（覆盖插件默认级别）
  useEffect(() => {
    invokeSetLogLevel(level).catch((e) => {
      console.warn("[myriad-mind] 初始化日志级别失败", e);
    });
    // 仅 mount 一次：后续切换由 setLogLevel 主动下发
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const setLogLevel = useCallback((next: LogLevel) => {
    setLevel(next);
    try { localStorage.setItem(STORAGE_KEY, next); } catch { /* ignore */ }
    invokeSetLogLevel(next).catch((e) => {
      console.warn("[myriad-mind] 切换日志级别失败", e);
    });
  }, []);

  return { logLevel: level, setLogLevel } as const;
}
