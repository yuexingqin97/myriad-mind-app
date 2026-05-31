import { useMemo } from "react";
import { type MyriadMindConfig, classifyInput, estimateCost, type TokenEstimate } from "@myriad-mind/core";
import { Button, Input } from "@myriad-mind/ui";
import { usePipeline } from "@/hooks/usePipeline";
import { LogPanel } from "@/components/log/LogPanel";

// ---- Props ----

interface InputViewProps {
  config: MyriadMindConfig;
}

// ---- Platform display helpers ----

const PLATFORM_META: Record<string, { icon: string; label: string; color: string }> = {
  bilibili:     { icon: "📺", label: "B 站",       color: "#fb7299" },
  youtube:      { icon: "▶️", label: "YouTube",    color: "#ff4444" },
  douyin:       { icon: "🎵", label: "抖音",       color: "#111" },
  xiaohongshu:  { icon: "📕", label: "小红书",     color: "#ff2442" },
  article_url:  { icon: "📄", label: "文章",       color: "var(--brand-primary)" },
  local_video:  { icon: "🎬", label: "本地视频",   color: "#a855f7" },
  local_audio:  { icon: "🎵", label: "本地音频",   color: "#a855f7" },
  local_text:   { icon: "📝", label: "本地文档",   color: "#a855f7" },
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

  const showHints = !inputUrl.trim() && !processing && !status;

  return (
    <div className="view-container view-container-flex">
      <div className="view-header">
        <h2 className="view-title">📥 神识一扫，万物皆可为笔记</h2>
        <p className="view-subtitle">
          丢入视频链接 / 文章 URL / 本地文件路径，自动炼化为 AI 摘要 + Mermaid 图表 + 术语表 + 扩展资源的结构化学习笔记
        </p>

        <div className="input-area">
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
              title="留空自动分类 · 如 Rust / AI / Rust/异步编程"
            />
            <Button onClick={submit} disabled={!inputUrl.trim() || processing} loading={processing}>
              炼化
            </Button>
          </div>

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

      <LogPanel entries={logs} streamingText={streamingText} />
    </div>
  );
}
