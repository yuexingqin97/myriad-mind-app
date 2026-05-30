// ============================================================
// DepsPanel — 系统依赖检测面板
// ============================================================

import { useEffect, useState } from "react";
import * as api from "../api";
import type { DepResult } from "../api";

export function DepsPanel() {
  const [deps, setDeps] = useState<Record<string, DepResult>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const all = await api.detectAllDeps();
        setDeps(all);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  if (loading) {
    return (
      <div style={{ padding: 12, fontSize: 12, color: "var(--text-muted)" }}>
        🔍 正在检测系统依赖…
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ padding: 12, fontSize: 12, color: "#f87171" }}>
        ⚠️ 依赖检测失败: {error}
      </div>
    );
  }

  const entries = Object.entries(deps);
  if (entries.length === 0) return null;

  const allOk = entries.every(([, d]) => d.found);

  return (
    <div
      style={{
        marginBottom: 20,
        padding: 14,
        borderRadius: 10,
        background: "var(--bg-surface)",
        border: `1px solid ${allOk ? "rgba(74,222,128,0.3)" : "rgba(250,204,21,0.3)"}`,
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 10,
        }}
      >
        <h4
          style={{
            fontSize: 12,
            fontWeight: 600,
            color: "var(--text-secondary)",
            textTransform: "uppercase",
            letterSpacing: "0.05em",
            margin: 0,
          }}
        >
          🔧 系统依赖
        </h4>
        <span
          style={{
            fontSize: 11,
            color: allOk ? "#4ade80" : "#facc15",
          }}
        >
          {allOk ? "✅ 全部就绪" : "⚠️ 部分缺失"}
        </span>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))", gap: 6 }}>
        {entries.map(([key, dep]) => (
          <DepBadge key={key} name={dep.name} found={dep.found} version={dep.version} suggestion={dep.suggestion} />
        ))}
      </div>
    </div>
  );
}

function DepBadge({
  name, found, version, suggestion,
}: { name: string; found: boolean; version?: string; suggestion?: string }) {
  return (
    <div
      title={suggestion}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 6,
        padding: "6px 10px",
        borderRadius: 6,
        fontSize: 11,
        background: found ? "rgba(74,222,128,0.08)" : "rgba(250,204,21,0.08)",
        border: `1px solid ${found ? "rgba(74,222,128,0.2)" : "rgba(250,204,21,0.2)"}`,
        color: found ? "#4ade80" : "#facc15",
      }}
    >
      <span>{found ? "✅" : "⚠️"}</span>
      <span style={{ fontWeight: 500 }}>{name}</span>
      {version && (
        <span style={{ color: "var(--text-muted)", fontSize: 10 }}>{version}</span>
      )}
    </div>
  );
}
