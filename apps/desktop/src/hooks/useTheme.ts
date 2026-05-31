import { useState, useEffect, useCallback } from "react";

// ---- Types ----

export type ThemeMode = "light" | "dark" | "system";

const STORAGE_KEY = "myriad-mind-theme";
const DEFAULT_THEME: ThemeMode = "dark";

// ---- Helpers ----

function getSystemTheme(): "light" | "dark" {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function resolveTheme(mode: ThemeMode): "light" | "dark" {
  return mode === "system" ? getSystemTheme() : mode;
}

function readStoredTheme(): ThemeMode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "light" || stored === "dark" || stored === "system") return stored;
  } catch { /* ignore */ }
  return DEFAULT_THEME;
}

function applyTheme(mode: ThemeMode) {
  if (typeof document === "undefined") return;
  const resolved = resolveTheme(mode);
  document.documentElement.dataset.theme = resolved;
}

// ---- Hook ----

export function useTheme() {
  const [mode, setMode] = useState<ThemeMode>(readStoredTheme);

  // Apply on mount
  useEffect(() => {
    applyTheme(mode);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Listen for system theme changes when mode === "system"
  useEffect(() => {
    if (mode !== "system") return;

    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const handler = () => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [mode]);

  const setTheme = useCallback((next: ThemeMode) => {
    setMode(next);
    applyTheme(next);
    try { localStorage.setItem(STORAGE_KEY, next); } catch { /* ignore */ }
  }, []);

  return {
    theme: mode,
    resolved: resolveTheme(mode),
    setTheme,
  } as const;
}
