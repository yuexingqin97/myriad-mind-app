import React, { useEffect, useRef } from "react";

// ---- Log entry types ----

export type LogEntryType = "info" | "step" | "output" | "success" | "error" | "divider";

export interface LogEntry {
  id: number;
  type: LogEntryType;
  text: string;
  timestamp: number;
}

// ---- Color map ----

const TYPE_STYLES: Record<LogEntryType, React.CSSProperties> = {
  info:    { color: "var(--text-secondary)" },
  step:    { color: "var(--brand-hover)",  fontWeight: 600 },
  output:  { color: "#c8c8e0" },
  success: { color: "var(--success)" },
  error:   { color: "var(--danger)" },
  divider: { color: "var(--border-default)" },
};

const TYPE_PREFIX: Record<LogEntryType, string> = {
  info:    "ℹ",
  step:    "▶",
  output:  " ",
  success: "✔",
  error:   "✖",
  divider: "─",
};

// ---- Props ----

interface LogPanelProps {
  entries: LogEntry[];
  /** Live streaming text from AI (appended after last entry) */
  streamingText?: string;
}

// ---- Component ----

export function LogPanel({ entries, streamingText }: LogPanelProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // auto scroll — instant, no animation queuing
    bottomRef.current?.scrollIntoView({ behavior: "auto" });
  }, [entries, streamingText]);

  return (
    <div className="log-panel">
      <div className="log-panel-header">
        <span className="log-panel-title">📜 炼化日志</span>
        <span className="log-panel-badge">{entries.length} 行</span>
      </div>

      <div className="log-panel-body">
        {entries.length === 0 && !streamingText && (
          <div className="log-empty">
            等待炼化…<br />
            <span style={{ fontSize: 11, opacity: 0.5 }}>
              日志将在此处实时展示 AI 输出内容和处理进度
            </span>
          </div>
        )}

        {entries.map((entry) => {
          if (entry.type === "divider") {
            return (
              <div key={entry.id} className="log-line log-line-divider">
                {"─".repeat(60)}
              </div>
            );
          }

          const ts = formatTime(entry.timestamp);
          return (
            <div key={entry.id} className={`log-line log-line-${entry.type}`}>
              <span className="log-ts">{ts}</span>
              <span className="log-prefix">{TYPE_PREFIX[entry.type]}</span>
              <span style={TYPE_STYLES[entry.type]}>{entry.text}</span>
            </div>
          );
        })}

        {/* Streaming AI output — live text */}
        {streamingText && (
          <div className="log-line log-line-output log-streaming">
            <span className="log-ts">{formatTime(Date.now())}</span>
            <span className="log-prefix">✦</span>
            <span style={{ color: "#d4b8ff" }}>
              {streamingText}
              <span className="log-cursor" />
            </span>
          </div>
        )}

        <div ref={bottomRef} />
      </div>
    </div>
  );
}

// ---- Helpers ----

function formatTime(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}
