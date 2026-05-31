// ============================================================
// DepsPanel — 系统依赖检测面板
// 优先使用配置中的 python_path，回退到自动探测
// ============================================================

import { useEffect, useState } from "react";
import * as api from "@/api";
import type { DepResult } from "@/api";

interface DepsPanelProps {
  /** 配置中的 Python 路径，传给 Rust detect_all_deps */
  pythonPath?: string;
}

export function DepsPanel({ pythonPath }: DepsPanelProps) {
  const [deps, setDeps] = useState<Record<string, DepResult>>({});
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        const all = await api.detectAllDeps(pythonPath || undefined);
        setDeps(all);
      } catch {
        // 检测失败时保持空结果，面板用灰色展示"未检测"
      } finally {
        setLoading(false);
      }
    })();
  }, [pythonPath]);

  // 固定的 5 项依赖列表 — 始终渲染，加载中/失败时用灰色状态
  const ALL_DEPS = [
    { key: "python", label: "Python" },
    { key: "ffmpeg", label: "FFmpeg" },
    { key: "faster-whisper", label: "faster-whisper" },
    { key: "yt-dlp", label: "yt-dlp" },
    { key: "gpu", label: "GPU/CUDA" },
  ] as const;

  const entries = Object.entries(deps);
  const allOk = entries.length > 0 && entries.every(([, d]) => d.found);

  return (
    <div
      style={{
        marginBottom: 12,
        padding: "6px 10px",
        borderRadius: 6,
        background: "var(--bg-surface)",
        border: `1px solid ${
          loading ? "var(--border-default)"
          : allOk ? "rgba(74,222,128,0.25)"
          : "rgba(250,204,21,0.25)"
        }`,
        display: "flex",
        alignItems: "center",
        gap: 10,
      }}
    >
      <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
        🔧 系统依赖
      </span>

      <div style={{ display: "flex", gap: 6, flexWrap: "wrap", alignItems: "center", flex: 1 }}>
        {ALL_DEPS.map(({ key, label }) => {
          const dep = deps[key];
          return (
            <DepBadge
              key={key}
              name={label}
              found={dep?.found}
              version={dep?.version}
              loading={loading}
            />
          );
        })}
      </div>

      <span style={{ fontSize: 10, color: loading ? "var(--text-muted)" : allOk ? "#4ade80" : "#facc15", whiteSpace: "nowrap" }}>
        {loading ? "⏳ 检测中…" : allOk ? "✅" : "⚠️"}
      </span>
    </div>
  );
}

function DepBadge({
  name, found, version, loading,
}: { name: string; found?: boolean; version?: string; loading?: boolean }) {
  const ok = found === true;
  const unknown = found === undefined || loading;
  const icon = unknown ? "⏳" : ok ? "✅" : "⚠️";
  const color = unknown ? "var(--text-muted)" : ok ? "#4ade80" : "#facc15";

  return (
    <span style={{ fontSize: 11, color, whiteSpace: "nowrap" }}>
      {icon} {name}
      {version && <span style={{ color: "var(--text-muted)", fontSize: 9, marginLeft: 2 }}> {version}</span>}
    </span>
  );
}
