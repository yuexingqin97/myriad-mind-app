import { useMemo, useState, useCallback, useRef } from "react";
import { type MyriadMindConfig, classifyInput, estimateCost, type TokenEstimate } from "@myriad-mind/core";
import { Button, Input } from "@myriad-mind/ui";
import { usePipeline } from "@/hooks/usePipeline";
import { LogPanel, type LogEntry } from "@/components/log/LogPanel";
import * as api from "@/api";
import { isTauri } from "@/lib/platform";

// ---- Props ----

interface InputViewProps {
  config: MyriadMindConfig;
}

// ---- Platform display helpers ----

const PLATFORM_META: Record<string, { icon: string; label: string; color: string }> = {
  bilibili: { icon: "📺", label: "B 站", color: "#fb7299" },
  youtube: { icon: "▶️", label: "YouTube", color: "#ff4444" },
  douyin: { icon: "🎵", label: "抖音", color: "#111" },
  xiaohongshu: { icon: "📕", label: "小红书", color: "#ff2442" },
  article_url: { icon: "📄", label: "文章", color: "var(--brand-primary)" },
  local_video: { icon: "🎬", label: "本地视频", color: "#a855f7" },
  local_audio: { icon: "🎵", label: "本地音频", color: "#a855f7" },
  local_text: { icon: "📝", label: "本地文档", color: "#a855f7" },
};

/** 平台名 → meta fallback（classify.platform 返回的是中文名） */
const PLATFORM_NAME_MAP: Record<string, string> = {
  "B 站": "bilibili",
  "YouTube": "youtube",
  "抖音/TikTok": "douyin",
  "小红书": "xiaohongshu",
  "知乎": "article_url",
  "CSDN": "article_url",
  "掘金": "article_url",
  "简书": "article_url",
  "微信公众号": "article_url",
  "Wiki": "article_url",
  "GitHub": "article_url",
  "通用": "article_url",
};

function resolveMeta(classify: { mode: string; platform: string }) {
  // 优先用 mode 查找（稳定标识符）
  if (PLATFORM_META[classify.mode]) return PLATFORM_META[classify.mode];
  // 回退：用 platform 中文名映射
  const mapped = PLATFORM_NAME_MAP[classify.platform];
  if (mapped) return PLATFORM_META[mapped];
  // 最终回退
  return PLATFORM_META["article_url"];
}

const MODE_LABELS: Record<string, string> = {
  bilibili: "视频炼化", youtube: "视频炼化", douyin: "视频炼化", xiaohongshu: "视频炼化",
  article_url: "文章炼化", local_video: "本地视频", local_audio: "本地音频",
  local_text: "文档解析", code_project: "代码分析",
};

function getCostLevel(estimate: TokenEstimate): { level: string; emoji: string; color: string } {
  if (estimate.estimatedMinutes < 5) return { level: "低", emoji: "🟢", color: "#4ade80" };
  if (estimate.estimatedMinutes < 15) return { level: "中", emoji: "🟡", color: "#facc15" };
  return { level: "高", emoji: "🔴", color: "#f87171" };
}

// ---- 支持的输入类型（只读标签，不提供手动选择）----

const SUPPORTED_INPUTS = [
  { icon: "📺", label: "B 站视频" },
  { icon: "▶️", label: "YouTube" },
  { icon: "🎵", label: "抖音" },
  { icon: "📕", label: "小红书" },
  { icon: "💬", label: "知乎" },
  { icon: "📰", label: "CSDN / 掘金" },
  { icon: "💚", label: "微信公众号" },
  { icon: "🌐", label: "任意网页" },
  { icon: "📁", label: "本地文件" },
  { icon: "💻", label: "代码仓库" },
];

// ---- Component ----

export function InputView({ config }: InputViewProps) {
  const {
    inputUrl,
    setInputUrl,
    noteCategory,
    setNoteCategory,
    taskPrompt,
    setTaskPrompt,
    status,
    progress,
    progressDetail,
    processing,
    logs,
    streamingText,
    submit,
  } = usePipeline({ config });

  // 实时预览：输入 URL 后即时显示分类和预估
  const preview = useMemo(() => {
    if (!inputUrl.trim()) return null;
    try {
      const classify = classifyInput(inputUrl.trim());
      const estimate = estimateCost(classify, config);
      const meta = resolveMeta(classify);
      const cost = getCostLevel(estimate);
      return { classify, estimate, meta, cost };
    } catch {
      return null;
    }
  }, [inputUrl, config]);

  const [mode, setMode] = useState<"new" | "qa">("new");
  const [qaNotePath, setQaNotePath] = useState("");
  const [qaQuestion, setQaQuestion] = useState("");
  const [qaProcessing, setQaProcessing] = useState(false);
  const [qaLogs, setQaLogs] = useState<LogEntry[]>([]);
  const [qaAnswer, setQaAnswer] = useState("");
  const qaLogIdRef = useRef(0);

  const pushQaLog = useCallback((type: LogEntry["type"], text: string) => {
    qaLogIdRef.current += 1;
    setQaLogs((prev) => [...prev, { id: qaLogIdRef.current, type, text, timestamp: Date.now() }]);
  }, []);

  const handleQaSubmit = useCallback(async () => {
    if (!qaNotePath.trim() || !qaQuestion.trim() || qaProcessing) return;
    setQaProcessing(true);
    setQaLogs([]);
    qaLogIdRef.current = 0;
    pushQaLog("step", `📖 读取笔记: ${qaNotePath}`);
    pushQaLog("step", `❓ 提问: ${qaQuestion}`);

    if (await isTauri()) {
      const unlisten = api.listenPipelineProgress((event) => {
        if (event.status === "running") pushQaLog("step", event.label);
        if (event.detail) pushQaLog("info", event.detail);
        if (event.status === "completed") pushQaLog("success", `✅ ${event.label}`);
      });
      try {
        const answer = await api.executeQa(qaNotePath.trim(), qaQuestion.trim(), true);
        setQaAnswer(answer);
        pushQaLog("output", answer);
        pushQaLog("success", "✅ 回答已追加到笔记");
      } catch (e) {
        pushQaLog("error", `❌ ${e}`);
      } finally {
        setQaProcessing(false);
        unlisten();
      }
    } else {
      // Mock
      await new Promise((r) => setTimeout(r, 1500));
      const mock = "（浏览器模拟）基于笔记内容的回答：根据笔记内容，最值得复用的方法是...";
      setQaAnswer(mock);
      pushQaLog("output", mock);
      pushQaLog("success", "✅ 回答已追加到笔记");
      setQaProcessing(false);
    }
  }, [qaNotePath, qaQuestion, qaProcessing, pushQaLog]);

  const showHints = mode === "new" && !inputUrl.trim() && !processing && !status;
  const effectiveLogs = mode === "new" ? logs : qaLogs;
  const effectiveStreaming = mode === "new" ? streamingText : qaAnswer;

  return (
    <div className="view-container view-container-flex">
      <div className="view-header">
        <h2 className="view-title">📥 神识一扫，万物皆可为笔记</h2>

        {/* 模式切换 */}
        <div style={{ display: "flex", gap: 4, marginBottom: 16 }}>
          <button
            onClick={() => setMode("new")}
            style={{
              padding: "6px 16px", fontSize: 13, fontWeight: 500, borderRadius: 8, border: "none", cursor: "pointer",
              background: mode === "new" ? "var(--brand-soft, rgba(22,131,255,0.14))" : "transparent",
              color: mode === "new" ? "var(--brand-primary, #1683ff)" : "var(--text-secondary, #a0a0c0)",
            }}
          >
            🆕 新炼化
          </button>
          <button
            onClick={() => setMode("qa")}
            style={{
              padding: "6px 16px", fontSize: 13, fontWeight: 500, borderRadius: 8, border: "none", cursor: "pointer",
              background: mode === "qa" ? "var(--brand-soft, rgba(22,131,255,0.14))" : "transparent",
              color: mode === "qa" ? "var(--brand-primary, #1683ff)" : "var(--text-secondary, #a0a0c0)",
            }}
          >
            💬 追问笔记
          </button>
        </div>

        <div className="input-area">
          {mode === "new" ? (
            <>
              <div className="input-row">
                <Input
                  placeholder="粘贴链接… 如 https://www.bilibili.com/video/BVxxx/"
                  value={inputUrl}
                  onChange={(e) => setInputUrl(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && submit()}
                />
                <input
                  placeholder="子目录（可选）"
                  value={noteCategory}
                  onChange={(e) => setNoteCategory(e.target.value)}
                  disabled={processing}
                  style={{
                    width: 130, padding: "8px 10px", fontSize: 13, borderRadius: 8,
                    border: "1px solid var(--border-default, #383a43)",
                    background: "var(--bg-input, #1f2026)",
                    color: "var(--text, #e0e0f0)", outline: "none",
                  }}
                  title="留空自动分类"
                />
                <Button onClick={submit} disabled={!inputUrl.trim() || processing} loading={processing}>
                  炼化
                </Button>
              </div>
              <TaskPromptInput value={taskPrompt} onChange={setTaskPrompt} disabled={processing} />
            </>
          ) : (
            <>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                <input
                  placeholder="笔记路径… 如 D:/Notes/MyriadMind/Rust/xxx.md"
                  value={qaNotePath}
                  onChange={(e) => setQaNotePath(e.target.value)}
                  disabled={qaProcessing}
                  style={{
                    padding: "10px 12px", fontSize: 13, borderRadius: 8,
                    border: "1px solid var(--border-default, #383a43)",
                    background: "var(--bg-input, #1f2026)", color: "var(--text, #e0e0f0)", outline: "none",
                  }}
                />
                <div style={{ display: "flex", gap: 8 }}>
                  <input
                    placeholder="你想追问什么？"
                    value={qaQuestion}
                    onChange={(e) => setQaQuestion(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleQaSubmit()}
                    disabled={qaProcessing}
                    style={{
                      flex: 1, padding: "10px 12px", fontSize: 13, borderRadius: 8,
                      border: "1px solid var(--border-default, #383a43)",
                      background: "var(--bg-input, #1f2026)", color: "var(--text, #e0e0f0)", outline: "none",
                    }}
                  />
                  <Button onClick={handleQaSubmit} disabled={!qaNotePath.trim() || !qaQuestion.trim() || qaProcessing} loading={qaProcessing}>
                    追问
                  </Button>
                </div>
              </div>
            </>
          )}

          {/* ---- 支持的输入内容（仅空输入时显示）---- */}
          {showHints && (
            <div className="supported-inputs">
              <span className="supported-inputs-label">支持识别并自动炼化：</span>
              <div className="supported-inputs-list">
                {SUPPORTED_INPUTS.map((item) => (
                  <span key={item.label} className="supported-input-badge">
                    <span style={{ fontSize: 14 }}>{item.icon}</span>
                    {item.label}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* ---- 实时预估卡片（输入后、提交前）---- */}
          {preview && !processing && (
            <div className="input-preview-card">
              <div className="input-preview-header">
                <span style={{ fontSize: 20 }}>{preview.meta.icon}</span>
                <span style={{ fontWeight: 600, color: preview.meta.color }}>{preview.meta.label}</span>
                <span className="input-preview-mode">{MODE_LABELS[preview.classify.mode] ?? preview.classify.mode}</span>
              </div>
              <div className="input-preview-grid">
                <div className="input-preview-item">
                  <span className="input-preview-value">{preview.estimate.estimatedMinutes}</span>
                  <span className="input-preview-label">预估耗时（分钟）</span>
                </div>
                <div className="input-preview-item">
                  <span className="input-preview-value">{Math.round((preview.estimate.inputTokens + preview.estimate.outputTokens) / 1000)}k</span>
                  <span className="input-preview-label">预估 Token</span>
                </div>
                <div className="input-preview-item">
                  <span className="input-preview-value" style={{ color: preview.cost.color }}>
                    {preview.cost.emoji} {preview.cost.level}
                  </span>
                  <span className="input-preview-label">灵力消耗等级</span>
                </div>
              </div>
            </div>
          )}

          {/* ---- 炼化进行中状态 ---- */}
          {processing && (
            <div className="input-processing-card">
              <div className="input-processing-top">
                <div className="input-processing-header">
                  <span className="input-processing-spinner" />
                  <span style={{ fontWeight: 600, fontSize: 14 }}>炼化中…</span>
                  <span style={{ fontSize: 13, color: "var(--brand-hover)", fontWeight: 600 }}>{progress}%</span>
                </div>
                <div style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: 4 }}>
                  {status}
                </div>
              </div>
              <div className="progress-bar input-processing-bar">
                <div className="progress-fill" style={{ width: `${progress}%` }} />
              </div>
              {progressDetail && (
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>
                  {progressDetail}
                </p>
              )}
            </div>
          )}

          {/* ---- 完成状态 ---- */}
          {status && !processing && (
            <div className="status-bar">
              <span>{status}</span>
              {progressDetail && (
                <p style={{ fontSize: 11, color: "var(--text-muted)", margin: "4px 0 0" }}>{progressDetail}</p>
              )}
            </div>
          )}
        </div>
      </div>

      <LogPanel entries={effectiveLogs} streamingText={effectiveStreaming} />
    </div>
  );
}

// ---- 本次要求组件 ----

const QUICK_TEMPLATES = [
  { label: "省流速览", text: "只输出重点摘要和结论，跳过细枝末节。" },
  { label: "教程步骤", text: "这是操作教程，请按步骤保留关键操作和注意事项。" },
  { label: "源码分析", text: "重点分析架构、模块职责、关键调用链和可复用设计。" },
  { label: "考试复习", text: "输出适合复习的提纲、术语表和自测题。" },
  { label: "只要结论", text: "优先给结论，再列必要依据。" },
];

function TaskPromptInput({ value, onChange, disabled }: { value: string; onChange: (v: string) => void; disabled: boolean }) {
  const [open, setOpen] = useState(false);

  return (
    <div style={{ marginTop: 8 }}>
      <button
        onClick={() => setOpen(!open)}
        disabled={disabled}
        style={{
          background: "none", border: "none", cursor: "pointer", padding: 0,
          fontSize: 12, color: "var(--text-muted, #666)", display: "flex", alignItems: "center", gap: 4,
        }}
      >
        <span>{open ? "▾" : "▸"}</span>
        <span>本次要求（可选）</span>
        {value && <span style={{ color: "var(--brand-primary, #1683ff)", fontSize: 11 }}>已填写</span>}
      </button>
      {open && (
        <div style={{ marginTop: 6, display: "flex", flexDirection: "column", gap: 6 }}>
          <textarea
            value={value}
            onChange={(e) => onChange(e.target.value)}
            disabled={disabled}
            placeholder="你希望这次怎么炼化？例如：只要速览，跳过评论区；重点保留操作画面。"
            rows={3}
            style={{
              width: "100%", padding: "8px 10px", fontSize: 12, borderRadius: 8, resize: "vertical",
              border: "1px solid var(--border-default, #383a43)", background: "var(--bg-input, #1f2026)",
              color: "var(--text, #e0e0f0)", outline: "none", fontFamily: "inherit",
            }}
          />
          <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
            {QUICK_TEMPLATES.map((t) => (
              <button
                key={t.label}
                disabled={disabled}
                onClick={() => onChange(value ? `${value} ${t.text}` : t.text)}
                style={{
                  padding: "2px 8px", fontSize: 11, borderRadius: 4, cursor: "pointer",
                  border: "1px solid var(--border-default, #383a43)", background: "var(--bg-surface, #1a1a2e)",
                  color: "var(--text-secondary, #aaa)",
                }}
              >
                {t.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
