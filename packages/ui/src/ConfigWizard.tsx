// ============================================================
// ConfigWizard — 多步骤配置引导
// 新步骤: welcome → deps → keys → processing → output → features → review
// 保存策略：每步只改 local state，最后一步「完成并保存」统一落盘
// ============================================================

import React, { useState } from "react";
import type { MyriadMindConfig } from "@myriad-mind/core";
import { Button } from "./common/Button.js";
import { Card } from "./common/Card.js";
import { Input, Select, Toggle } from "./common/Input.js";
import type { DepsInfo, SetupIntent } from "./types.js";

export interface ConfigWizardProps {
  config: MyriadMindConfig;
  /** 保存回调 — action: "go_input" 跳转炼化页, "stay_settings" 留在设置 */
  onSave: (config: MyriadMindConfig, action: "go_input" | "stay_settings") => void;
  onCancel?: () => void;
  /** 系统依赖状态 */
  deps?: DepsInfo;
  /** 重新检测依赖回调 */
  onRecheckDeps?: () => void;
  /** 选择输出目录 */
  onSelectOutputDir?: () => Promise<string | null>;
  /** 打开输出目录 */
  onOpenOutputDir?: () => void;
  /** 打开外部链接（注册页等，调系统浏览器） */
  onOpenUrl?: (url: string) => void;
}

type StepId = "welcome" | "deps" | "keys" | "processing" | "output" | "features" | "review";

const STEPS: Array<{ id: StepId; title: string; icon: string }> = [
  { id: "welcome",    title: "使用路径",   icon: "🧭" },
  { id: "deps",       title: "环境检测",   icon: "🔧" },
  { id: "keys",       title: "AI 密钥",    icon: "🔑" },
  { id: "processing", title: "处理策略",   icon: "⚙️" },
  { id: "output",     title: "输出设置",   icon: "📂" },
  { id: "features",   title: "笔记偏好",   icon: "📝" },
  { id: "review",     title: "完成检查",   icon: "✅" },
];

function getBlockingDeps(deps: DepsInfo | undefined, intent: SetupIntent): string[] {
  if (!deps) return ["依赖检测未完成"];

  if (intent === "article" || intent === "code") return [];

  const missing: string[] = [];
  if (!deps.python?.found) missing.push("Python 3.9+");
  if (!deps.fasterWhisper?.found) missing.push("faster-whisper");
  if (intent === "video" && !deps.ytdlp?.found) missing.push("yt-dlp");
  if (!deps.ffmpeg?.found) missing.push("FFmpeg");
  return missing;
}

// ============================================================

export function ConfigWizard({
  config: initialConfig,
  onSave,
  onCancel,
  deps,
  onRecheckDeps,
  onSelectOutputDir,
  onOpenOutputDir,
  onOpenUrl,
}: ConfigWizardProps) {
  const [step, setStep] = useState(0);
  const [config, setConfig] = useState<MyriadMindConfig>({ ...initialConfig });
  const [setupIntent, setSetupIntent] = useState<SetupIntent>("video");

  const update = <K extends keyof MyriadMindConfig>(
    key: K,
    value: MyriadMindConfig[K],
  ) => setConfig((c) => ({ ...c, [key]: value }));

  const current = STEPS[step];
  const isFirst = step === 0;
  const isLast = step === STEPS.length - 1;

  // Step-specific "next" availability
  const canProceed = (() => {
    if (step === 1) return getBlockingDeps(deps, setupIntent).length === 0;
    if (step === 4) return !!config.output.note_dir?.trim(); // output dir required
    return true;
  })();
  const blockingDeps = getBlockingDeps(deps, setupIntent);

  return (
    <Card
      title="⚙️ 配置向导"
      subtitle={`步骤 ${step + 1}/${STEPS.length}: ${current.title}`}
      variant="bordered"
      footer={
        <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
          <div style={{ display: "flex", gap: 8 }}>
            {!isFirst && (
              <Button variant="secondary" onClick={() => setStep((s) => s - 1)}>
                ← 上一步
              </Button>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            {onCancel && !isLast && (
              <Button variant="ghost" onClick={onCancel}>取消</Button>
            )}
            {isLast ? (
              <>
                <Button variant="secondary" onClick={() => setStep((s) => s - 1)}>← 返回修改</Button>
                <Button variant="secondary" onClick={() => onSave(config, "stay_settings")}>💾 保存后留在设置</Button>
                <Button onClick={() => onSave(config, "go_input")}>🚀 完成并开始炼化</Button>
              </>
            ) : (
              <Button
                onClick={() => setStep((s) => s + 1)}
                disabled={!canProceed}
                title={step === 1 && blockingDeps.length ? `缺少：${blockingDeps.join("、")}` : undefined}
              >
                下一步 →
              </Button>
            )}
          </div>
        </div>
      }
    >
      {/* 进度条 */}
      <div style={{ display: "flex", gap: 3, marginBottom: 24 }}>
        {STEPS.map((s, i) => (
          <div
            key={s.id}
            title={s.title}
            style={{
              height: 4,
              borderRadius: 2,
              flex: 1,
              background: i <= step ? "var(--brand-primary)" : "var(--border, #2a2a4a)",
              transition: "background 0.3s",
            }}
          />
        ))}
      </div>

      {/* 步骤内容 */}
      <div style={{ minHeight: 300 }}>
        {step === 0 && <WelcomeStep intent={setupIntent} onChange={setSetupIntent} />}
        {step === 1 && <DepsStep deps={deps} intent={setupIntent} onRecheck={onRecheckDeps} />}
        {step === 2 && <ApiKeysStep config={config} update={update} onOpenUrl={onOpenUrl} />}
        {step === 3 && <ProcessingStep config={config} update={update} intent={setupIntent} />}
        {step === 4 && <OutputStep config={config} update={update} onSelectDir={onSelectOutputDir} />}
        {step === 5 && <FeaturesStep config={config} update={update} />}
        {step === 6 && <ReviewStep config={config} intent={setupIntent} deps={deps} />}
      </div>
    </Card>
  );
}

// ============================================================
// Step 0: Welcome — 选择使用路径
// ============================================================

const INTENT_OPTIONS: Array<{ value: SetupIntent; icon: string; label: string; desc: string; available: boolean }> = [
  { value: "video",       icon: "🎬", label: "在线视频",   desc: "B 站 / YouTube / 抖音 / 小红书", available: true },
  { value: "local_media", icon: "📁", label: "本地媒体",   desc: "本地视频 / 音频文件",             available: true },
  { value: "article",     icon: "📄", label: "网页文章",   desc: "知乎 / CSDN / 掘金 / 普通网页",   available: true },
  { value: "code",        icon: "💻", label: "代码项目",   desc: "GitHub / 本地目录（稍后支持）",   available: false },
];

function WelcomeStep({ intent, onChange }: { intent: SetupIntent; onChange: (v: SetupIntent) => void }) {
  return (
    <div>
      <p style={{ fontSize: 14, color: "var(--text-secondary, #a0a0c0)", marginBottom: 20, lineHeight: 1.6 }}>
        你主要想处理什么内容？选择只是为了优化默认配置，之后仍可处理其他类型。
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))", gap: 10 }}>
        {INTENT_OPTIONS.map((opt) => (
          <div
            key={opt.value}
            onClick={() => opt.available && onChange(opt.value)}
            title={!opt.available ? "v1 暂不支持，敬请期待" : undefined}
            style={{
              display: "flex", alignItems: "center", gap: 14,
              padding: 18, borderRadius: 10,
              border: intent === opt.value ? "2px solid var(--brand-primary)" : "1px solid var(--border, #2a2a4a)",
              background: intent === opt.value ? "rgba(99,102,241,0.1)" : "var(--bg-surface, #1a1a2e)",
              cursor: opt.available ? "pointer" : "not-allowed",
              opacity: opt.available ? 1 : 0.45,
              transition: "all 0.15s",
            }}
          >
            <span style={{ fontSize: 28 }}>{opt.icon}</span>
            <div>
              <p style={{ fontSize: 14, fontWeight: 600, color: "var(--text, #e0e0f0)", margin: 0 }}>{opt.label}</p>
              <p style={{ fontSize: 12, color: "var(--text-muted, #666)", margin: 0, marginTop: 2 }}>
                {opt.desc}
              </p>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ============================================================
// Step 1: Deps — 系统依赖检查
// ============================================================

// ---- 依赖详情配置 ----

const DEP_DETAILS: Array<{
  key: string; label: string; icon: string;
  usage: string;         // 用途说明
  fixHint: string;       // 如何安装
  critical: boolean;
}> = [
  {
    key: "python", label: "Python", icon: "🐍", critical: true,
    usage: "运行 ASR 转写（faster-whisper）、视频截图、调度脚本。整个管线引擎依赖 Python 环境。",
    fixHint: "安装 Python 3.9+ → https://python.org 或使用 winget install Python.Python.3.12",
  },
  {
    key: "ffmpeg", label: "FFmpeg", icon: "🎞️", critical: false,
    usage: "视频/音频解码、提取音频流、关键帧截图。处理任何视频都需要 FFmpeg。",
    fixHint: "winget install FFmpeg 或从 https://ffmpeg.org 下载，确保 ffmpeg 在 PATH 中",
  },
  {
    key: "fasterWhisper", label: "faster-whisper", icon: "🎙️", critical: false,
    usage: "本地 ASR 转写。没有可用字幕时，音视频会回退到 faster-whisper 生成字幕。",
    fixHint: "使用应用检测到的 Python 执行：python -m pip install -U faster-whisper",
  },
  {
    key: "ytdlp", label: "yt-dlp", icon: "⬇️", critical: false,
    usage: "下载 YouTube / B 站等在线视频、提取字幕。YouTube 视频的自动字幕优先策略依赖 yt-dlp。",
    fixHint: "winget install yt-dlp.yt-dlp 或 pip install yt-dlp",
  },
  {
    key: "gpu", label: "GPU/CUDA", icon: "🖥️", critical: false,
    usage: "加速本地 ASR 转写（faster-whisper 在 GPU 上快 5-10 倍）。没有 GPU 也能用 CPU 转写，只是较慢。",
    fixHint: "安装 NVIDIA CUDA Toolkit 或使用 CPU 模式（自动降级，不影响使用）",
  },
];

function DepsStep({ deps, intent, onRecheck }: { deps?: DepsInfo; intent: SetupIntent; onRecheck?: () => void }) {
  if (!deps) {
    return (
      <div style={{ textAlign: "center", padding: 40 }}>
        <p style={{ color: "var(--text-muted, #666)", fontSize: 13 }}>系统依赖检测未就绪</p>
        <p style={{ color: "var(--text-muted, #666)", fontSize: 12, marginTop: 8 }}>
          请确保应用已连接后端进行检测
        </p>
      </div>
    );
  }

  const depMap: Record<string, typeof deps.python> = {
    python: deps.python,
    ffmpeg: deps.ffmpeg,
    fasterWhisper: deps.fasterWhisper,
    ytdlp: deps.ytdlp,
    gpu: deps.gpu,
  };

  const blockingDeps = getBlockingDeps(deps, intent);
  const missingCritical = blockingDeps.length > 0;
  const missingOptional = DEP_DETAILS.some((d) => !depMap[d.key]?.found);

  return (
    <div>
      <p style={{ fontSize: 14, color: "var(--text-secondary, #a0a0c0)", marginBottom: 16, lineHeight: 1.6 }}>
        大衍决会在本机完成下载、转写和截图。你当前选择的是 <strong>{intent === "video" ? "在线视频" : intent === "local_media" ? "本地媒体" : "文章/代码"}</strong> 路径；所需依赖不齐时不会进入下一步。
      </p>

      {/* Status summary */}
      <div style={{
        marginBottom: 16, padding: "12px 14px", borderRadius: 8, fontSize: 13,
        background: missingCritical ? "rgba(248,113,113,0.08)" : missingOptional ? "rgba(250,204,21,0.08)" : "rgba(74,222,128,0.08)",
        border: `1px solid ${missingCritical ? "rgba(248,113,113,0.25)" : missingOptional ? "rgba(250,204,21,0.25)" : "rgba(74,222,128,0.25)"}`,
        color: missingCritical ? "#f87171" : missingOptional ? "#facc15" : "#4ade80",
        lineHeight: 1.6,
      }}>
        {missingCritical
          ? `❌ 当前路径缺少必需依赖：${blockingDeps.join("、")}。请补齐后重新检测。`
          : missingOptional
            ? "⚠️ 部分工具缺失 — 视频下载和转写功能受限，文章处理正常。"
            : "✅ 全部依赖就绪 — 可处理文章、视频和本地文件"}
      </div>

      {/* Individual deps — detailed cards */}
      <div style={{ display: "flex", flexDirection: "column", gap: 8, marginBottom: 16 }}>
        {DEP_DETAILS.map(({ key, label, icon, usage, fixHint, critical }) => {
          const dep = depMap[key];
          const status = dep?.found ? "ok" : critical ? "error" : "warning";
          return (
            <div
              key={key}
              style={{
                padding: "12px 14px", borderRadius: 10,
                border: `1px solid ${status === "ok" ? "rgba(74,222,128,0.2)" : status === "error" ? "rgba(248,113,113,0.2)" : "rgba(250,204,21,0.2)"}`,
                background: status === "ok" ? "rgba(74,222,128,0.04)" : status === "error" ? "rgba(248,113,113,0.04)" : "rgba(250,204,21,0.04)",
              }}
            >
              <div style={{ display: "flex", alignItems: "flex-start", gap: 10 }}>
                <span style={{ fontSize: 22, lineHeight: 1 }}>{icon}</span>
                <div style={{ flex: 1 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                    <span style={{ fontWeight: 600, fontSize: 14, color: "var(--text, #e0e0f0)" }}>{label}</span>
                    <span style={{
                      fontSize: 11, fontWeight: 600,
                      padding: "1px 8px", borderRadius: 4,
                      background: status === "ok" ? "rgba(74,222,128,0.15)" : status === "error" ? "rgba(248,113,113,0.15)" : "rgba(250,204,21,0.15)",
                      color: status === "ok" ? "#4ade80" : status === "error" ? "#f87171" : "#facc15",
                    }}>
                      {status === "ok" ? "已就绪" : status === "error" ? "缺失" : "可选"}
                    </span>
                    {dep?.version && (
                      <span style={{ fontSize: 11, color: "var(--text-muted, #666)" }}>{dep.version}</span>
                    )}
                  </div>
                  <p style={{ fontSize: 12, color: "var(--text-secondary, #a0a0c0)", margin: "0 0 4px", lineHeight: 1.5 }}>{usage}</p>
                  {!dep?.found && (
                    <p style={{ fontSize: 11, color: status === "error" ? "#f87171" : "#facc15", margin: 0, lineHeight: 1.4 }}>
                      💡 {fixHint}
                    </p>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Actions */}
      <div style={{ display: "flex", gap: 8 }}>
        {onRecheck && (
          <Button variant="secondary" onClick={onRecheck}>🔄 重新检测</Button>
        )}
      </div>
    </div>
  );
}

// ============================================================
// Step 2: Keys — API 密钥（受控，最后一步统一保存）
// ============================================================

// ---- AI 模型提供商 ----

const AI_PROVIDERS = [
  { id: "deepseek", label: "DeepSeek", desc: "V4 Pro / Flash · 1M 上下文 · 主力模型", available: true },
  { id: "claude", label: "Claude", desc: "Anthropic · 待后续版本支持", available: false },
] as const;

function ApiKeysStep({
  config, update, onOpenUrl,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  onOpenUrl?: (url: string) => void;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* AI 模型选择 */}
      <div>
        <h4 style={{ fontSize: 14, fontWeight: 600, color: "var(--text, #e0e0f0)", marginBottom: 4 }}>
          🤖 AI 模型
        </h4>
        <p style={{ fontSize: 12, color: "var(--text-muted, #666)", marginBottom: 12 }}>
          选择笔记生成使用的 AI 模型，后续将支持更多提供商
        </p>
        <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
          {AI_PROVIDERS.map((p) => (
            <div
              key={p.id}
              title={!p.available ? "后续版本支持" : undefined}
              style={{
                flex: 1, padding: "12px 14px", borderRadius: 10,
                border: p.id === "deepseek" ? "2px solid #1683ff" : "1px solid var(--border, #2a2a4a)",
                background: p.id === "deepseek" ? "rgba(22,131,255,0.08)" : "var(--bg-surface, #1a1a2e)",
                opacity: p.available ? 1 : 0.4,
                cursor: p.available ? "default" : "not-allowed",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{ fontWeight: 600, fontSize: 14, color: p.available ? "var(--text, #e0e0f0)" : "var(--text-muted, #666)" }}>
                  {p.label}
                </span>
                {p.id === "deepseek" && <span style={{ fontSize: 10, padding: "1px 6px", borderRadius: 4, background: "rgba(22,131,255,0.15)", color: "#1683ff" }}>当前</span>}
                {!p.available && <span style={{ fontSize: 10, color: "var(--text-muted, #666)" }}>即将支持</span>}
              </div>
              <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "4px 0 0" }}>{p.desc}</p>
            </div>
          ))}
        </div>
        {/* DeepSeek API Key（必填，受控） */}
        <ApiKeyField
          label="DeepSeek API Key"
          description="platform.deepseek.com → API Keys · 必填"
          placeholder="sk-..."
          required
          value={config.deepseek_api_key ?? ""}
          onChange={(v) => update("deepseek_api_key", v)}
          link={{ url: "https://platform.deepseek.com/api_keys", label: "前往 DeepSeek 控制台 ↗" }}
          onOpenUrl={onOpenUrl}
        />
      </div>

      {/* 视频解析服务 */}
      <div>
        <h4 style={{ fontSize: 14, fontWeight: 600, color: "var(--text, #e0e0f0)", marginBottom: 12 }}>
          📡 视频解析服务
        </h4>
        <ApiKeyField
          label="AI Douyin API Key"
          description="抖音/B 站/小红书视频解析。ai-douyin.top9.cc 注册获取"
          placeholder="输入 AI Douyin API Key..."
          required={false}
          value={config.ai_douyin_api_key ?? ""}
          onChange={(v) => update("ai_douyin_api_key", v)}
          link={{ url: "https://ai-douyin.top9.cc/", label: "前往 ai-douyin.top9.cc ↗" }}
          onOpenUrl={onOpenUrl}
        />
      </div>
    </div>
  );
}

/** 受控 API Key 输入（编辑/确认/删除都改 config state，由向导最后一步统一保存） */
function ApiKeyField({
  label, description, placeholder, required, value, onChange, link, onOpenUrl,
}: {
  label: string; description: string; placeholder: string; required: boolean;
  value: string; onChange: (v: string) => void;
  link?: { url: string; label?: string };
  onOpenUrl?: (url: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  const masked = maskSecret(value);
  const status: "set" | "empty" = value ? "set" : "empty";

  const confirm = () => {
    if (!draft.trim()) return;
    onChange(draft.trim());
    setDraft("");
    setEditing(false);
  };

  const remove = () => {
    onChange("");
    setEditing(false);
    setDraft("");
  };

  return (
    <div style={{
      padding: 12, borderRadius: 8,
      border: `1px solid ${status === "set" ? "rgba(74,222,128,0.3)" : required ? "rgba(248,113,113,0.3)" : "rgba(250,204,21,0.3)"}`,
      background: "var(--bg-surface, rgba(255,255,255,0.02))",
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 4 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ fontWeight: 600, fontSize: 13 }}>{label}</span>
          {required && <span style={{ fontSize: 10, color: "#f87171" }}>必填</span>}
          {status === "set" && <span style={{ fontSize: 11, color: "#4ade80" }}>✅ 已配置</span>}
          {status === "empty" && !required && <span style={{ fontSize: 11, color: "#facc15" }}>可选</span>}
        </div>
        {!editing && (
          <button onClick={() => setEditing(true)} style={{
            fontSize: 11, padding: "2px 8px", borderRadius: 4,
            border: "1px solid var(--border, #333)", background: "transparent",
            color: "var(--text-secondary, #aaa)", cursor: "pointer",
          }}>
            {status === "set" ? "编辑" : "配置"}
          </button>
        )}
      </div>
      <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "0 0 4px" }}>{description}</p>
      {link && (
        <a
          href={link.url} target="_blank" rel="noopener noreferrer"
          onClick={onOpenUrl ? (e) => { e.preventDefault(); onOpenUrl(link.url); } : undefined}
          style={{ fontSize: 11, color: "var(--brand-primary, #1683ff)", textDecoration: "none", display: "inline-block", marginBottom: 4 }}
        >
          {link.label ?? link.url}
        </a>
      )}
      {!editing && status === "set" && (
        <p style={{ fontSize: 12, color: "var(--text-secondary, #aaa)", fontFamily: "monospace", margin: 0 }}>{masked}</p>
      )}
      {editing && (
        <div style={{ marginTop: 8, display: "flex", gap: 8, alignItems: "center" }}>
          <input type="password" value={draft} onChange={(e) => setDraft(e.target.value)}
            placeholder={placeholder} onKeyDown={(e) => e.key === "Enter" && confirm()} autoFocus
            style={{ flex: 1, padding: "6px 10px", fontSize: 12, borderRadius: 6, border: "1px solid var(--border, #333)", background: "var(--bg-app, #111)", color: "var(--text, #eee)", outline: "none" }} />
          <button onClick={confirm} disabled={!draft.trim()} style={{
            padding: "6px 12px", fontSize: 11, borderRadius: 6, border: "none", background: "var(--brand-primary)", color: "white", cursor: draft.trim() ? "pointer" : "not-allowed", opacity: draft.trim() ? 1 : 0.5,
          }}>保存</button>
          {status === "set" && (
            <button onClick={remove} style={{ padding: "6px 12px", fontSize: 11, borderRadius: 6, border: "1px solid rgba(248,113,113,0.3)", background: "transparent", color: "#f87171", cursor: "pointer" }}>删除</button>
          )}
          <button onClick={() => { setEditing(false); setDraft(""); }} style={{
            padding: "6px 12px", fontSize: 11, borderRadius: 6, border: "1px solid var(--border, #333)", background: "transparent", color: "var(--text-secondary, #aaa)", cursor: "pointer",
          }}>取消</button>
        </div>
      )}
    </div>
  );
}

function maskSecret(secret: string): string {
  if (!secret) return "";
  if (secret.length <= 16) return "••••••••";
  return secret.slice(0, 8) + "..." + secret.slice(-4);
}

// ============================================================
// Step 3: Processing — ASR + 视频策略 (合并)
// ============================================================

function ProcessingStep({
  config, update, intent,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  intent: SetupIntent;
}) {
  const showVideo = intent !== "article";
  const [showAsrAdvanced, setShowAsrAdvanced] = useState(false);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {/* ASR 配置（固定 faster-whisper 本地转写；云端 ASR 暂未支持） */}
      <div>
        <h4 style={{ fontSize: 13, fontWeight: 600, color: "var(--text, #e0e0f0)", marginBottom: 10 }}>
          🎙️ 语音识别
        </h4>
        <p style={{ fontSize: 11, color: "var(--text-muted, #666)", marginBottom: 10 }}>
          使用 faster-whisper 本地转写（免费、离线）。云端 ASR（火山引擎）暂未接入。
        </p>
        <div style={{ display: "flex", flexDirection: "column", gap: 10, paddingLeft: 4 }}>
          <button onClick={() => setShowAsrAdvanced(!showAsrAdvanced)} style={{
            background: "none", border: "none", color: "var(--brand-hover, var(--brand-hover))", fontSize: 12, cursor: "pointer", padding: 0, textAlign: "left",
          }}>
            {showAsrAdvanced ? "▾ 收起高级选项" : "▸ 展开高级选项（模型/设备）"}
          </button>
          {showAsrAdvanced && (
            <>
              <Select label="模型大小" value={config.asr.faster_whisper?.model_size ?? "small"}
                options={[
                  { value: "tiny", label: "tiny — 最小最快" },
                  { value: "base", label: "base — 基础" },
                  { value: "small", label: "small — 推荐" },
                  { value: "medium", label: "medium — 准确" },
                  { value: "large-v3", label: "large-v3 — 最准确（需大显存）" },
                ]}
                onChange={(e) => update("asr", {
                  backend: "faster-whisper",
                  faster_whisper: {
                    model_size: e.target.value as "tiny" | "base" | "small" | "medium" | "large-v3",
                    device: config.asr.faster_whisper?.device ?? "auto",
                    compute_type: config.asr.faster_whisper?.compute_type,
                  },
                })}
              />
              <div style={{ display: "flex", gap: 6 }}>
                {(["auto", "cpu", "cuda"] as const).map((d) => (
                  <Pill key={d} active={(config.asr.faster_whisper?.device ?? "auto") === d}
                    onClick={() => update("asr", {
                      backend: "faster-whisper",
                      faster_whisper: {
                        model_size: config.asr.faster_whisper?.model_size ?? "small",
                        device: d,
                        compute_type: config.asr.faster_whisper?.compute_type,
                      },
                    })}>
                    {d === "auto" ? "自动检测" : d === "cpu" ? "CPU" : "CUDA"}
                  </Pill>
                ))}
              </div>
            </>
          )}
        </div>
      </div>

      {/* 视频解析 */}
      {showVideo && (
        <div>
          <h4 style={{ fontSize: 13, fontWeight: 600, color: "var(--text, #e0e0f0)", marginBottom: 10 }}>
            📹 视频解析
          </h4>
          <div style={{ display: "flex", gap: 6, marginBottom: 8 }}>
            <Pill active={config.video.provider === "ai-douyin"} onClick={() => update("video", { provider: "ai-douyin" })}>
              AI Douyin（推荐）
            </Pill>
            <Pill active={config.video.provider === "tikhub"} onClick={() => update("video", { provider: "tikhub" })}>
              TikHub
            </Pill>
          </div>
          <p style={{ fontSize: 11, color: "var(--text-muted, #666)" }}>
            抖音/小红书/B 站视频解析需要 AI Douyin API Key（在「AI 密钥」步骤配置）。YouTube 不需要额外 Key。
          </p>
        </div>
      )}

      {!showVideo && (
        <p style={{ fontSize: 12, color: "var(--text-muted, #666)", padding: "12px 0" }}>
          📄 你选择了网页文章路径 — 视频解析能力可稍后在设置中配置。
        </p>
      )}
    </div>
  );
}

// ---- Pill button ----

function Pill({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: "6px 14px", fontSize: 12, fontWeight: 500, borderRadius: 8,
        color: active ? "var(--brand-hover)" : "var(--text-secondary, #a0a0c0)",
        background: active ? "rgba(99,102,241,0.15)" : "var(--bg-app, #111)",
        border: `1px solid ${active ? "rgba(99,102,241,0.35)" : "var(--border, #2a2a4a)"}`,
        cursor: "pointer", transition: "all 0.15s",
      }}
    >
      {children}
    </button>
  );
}

// ============================================================
// Step 4: Output — 输出设置
// ============================================================

function OutputStep({
  config, update, onSelectDir,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  onSelectDir?: () => Promise<string | null>;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <span style={{ fontSize: 12, color: "var(--text-secondary, #a0a0c0)", marginBottom: 4, display: "block" }}>
          笔记输出目录 *
        </span>
        <div style={{ display: "flex", gap: 8 }}>
          <input
            value={config.output.note_dir}
            onChange={(e) => update("output", { ...config.output, note_dir: e.target.value })}
            placeholder="必填 — 例如: D:/Notes/MyriadMind"
            style={{
              flex: 1, padding: "8px 12px", fontSize: 13, borderRadius: 8,
              border: "1px solid var(--border, #333)", background: "var(--bg-input, #1f2026)",
              color: "var(--text, #e0e0f0)", outline: "none",
            }}
          />
          {onSelectDir && (
            <button
              onClick={async () => {
                const dir = await onSelectDir();
                if (dir) update("output", { ...config.output, note_dir: dir });
              }}
              style={{
                padding: "8px 14px", fontSize: 13, borderRadius: 8, whiteSpace: "nowrap",
                border: "1px solid var(--border, #333)", background: "var(--bg-surface, #1a1a2e)",
                color: "var(--text-secondary, #aaa)", cursor: "pointer",
              }}
            >
              📁 选择目录
            </button>
          )}
        </div>
        <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "4px 0 0" }}>
          笔记将保存到此目录。推荐选云盘文件夹以实现跨设备同步
        </p>
        {!config.output.note_dir && (
          <p style={{ fontSize: 11, color: "#f87171", marginTop: 4 }}>⚠️ 必须设置输出目录，否则无法保存生成的笔记</p>
        )}
      </div>

      <Toggle
        label="自动清理临时文件"
        description="处理完成后删除 /tmp 中的视频、音频、字幕、截图。关闭此项可保留缓存视频（调试用）"
        checked={config.output.cleanup_temp}
        onChange={(v) => update("output", { ...config.output, cleanup_temp: v })}
      />
      <Toggle
        label="笔记末尾添加元信息"
        description="记录生成时间、模型、Token 消耗、更新日志等"
        checked={config.output.note_metadata}
        onChange={(v) => update("output", { ...config.output, note_metadata: v })}
      />
    </div>
  );
}

// ============================================================
// Step 5: Features — 笔记偏好 (含关键帧)
// ============================================================

function FeaturesStep({
  config, update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
}) {
  const toggle = (key: keyof typeof config.features) => {
    update("features", { ...config.features, [key]: !config.features[key] });
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <p style={{ fontSize: 13, color: "var(--text-secondary, #a0a0c0)", marginBottom: 4 }}>
        控制笔记生成时包含哪些内容（全部默认开启）
      </p>

      <Toggle label="Mermaid 图表" description="自动绘制架构图、流程图、时序图"
        checked={config.features.mermaid} onChange={() => toggle("mermaid")} />
      <Toggle label="扩展学习资源" description="推荐相关文档、视频、GitHub 仓库等"
        checked={config.features.resources} onChange={() => toggle("resources")} />
      <Toggle label="评论区精华" description="自动提取视频评论区高质量讨论"
        checked={config.features.comments} onChange={() => toggle("comments")} />
      <Toggle label="阅读时长与难度评级" description="在笔记开头标注推荐阅读时间和内容难度"
        checked={config.features.reading_info} onChange={() => toggle("reading_info")} />
      <Toggle label="灵力预估" description="处理前显示 Token/时间/费用预估"
        checked={config.features.estimation} onChange={() => toggle("estimation")} />

      {/* 关键帧（合并在 features 中） */}
      <div style={{ marginTop: 8 }}>
        <Toggle label="关键帧截图" description="从视频截图并嵌入笔记，帮助理解画面内容"
          checked={config.features.keyframes} onChange={() => toggle("keyframes")} />

        {config.features.keyframes && (
          <div style={{ marginTop: 12, marginLeft: 4, paddingLeft: 12, borderLeft: "2px solid var(--border, #2a2a4a)" }}>
            <Input
              label="最大截图数"
              type="number" min={5} max={100}
              value={config.keyframes.max_frames}
              onChange={(e) => update("keyframes", {
                ...config.keyframes,
                max_frames: Math.max(5, Math.min(100, Number(e.target.value) || 40)),
              })}
              hint="候选截图上限，AI 审查后仅保留高分帧"
            />
            <div style={{ marginTop: 10 }}>
              <Input
                label="场景检测灵敏度"
                type="number" min={0.05} max={1.0} step={0.05}
                value={config.keyframes.scene_threshold}
                onChange={(e) => update("keyframes", {
                  ...config.keyframes,
                  scene_threshold: Math.max(0.05, Math.min(1.0, Number(e.target.value) || 0.25)),
                })}
                hint="阈值越低越灵敏（更多帧），范围 0.05-1.0"
              />
            </div>
            <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "8px 0 0" }}>
              截图方式：AI 字幕分析推荐时间点 + 场景变化检测，仅 AI 视觉审查通过的截图进入笔记
            </p>
          </div>
        )}
      </div>

      {/* 收尾操作 */}
      <div style={{ marginTop: 8 }}>
        <Toggle label="自动更新修为面板" description="每次生成笔记后自动刷新统计数据和成就进度"
          checked={config.post_process.auto_update_panel}
          onChange={(v) => update("post_process", { ...config.post_process, auto_update_panel: v })} />
        <div style={{ marginTop: 10 }}>
          <Toggle label="学习路线推荐" description="基于当前知识结构推荐下一步学习方向"
            checked={config.post_process.auto_suggest_next}
            onChange={(v) => update("post_process", { ...config.post_process, auto_suggest_next: v })} />
        </div>
      </div>
    </div>
  );
}

// ============================================================
// Step 6: Review — 完成检查
// ============================================================

function ReviewStep({
  config, intent, deps,
}: {
  config: MyriadMindConfig;
  intent: SetupIntent;
  deps?: DepsInfo;
}) {
  const intentLabel = { video: "在线视频", local_media: "本地媒体", article: "网页文章", code: "代码项目" }[intent];

  const depsOk = deps
    ? Object.values(deps).every((d) => d.found)
    : null;

  const rows = [
    { label: "使用路径", value: intentLabel },
    { label: "系统依赖", value: depsOk === null ? "未检测" : depsOk ? "全部就绪" : "有缺失项", ok: depsOk },
    { label: "ASR 后端", value: "faster-whisper（本地）" },
    { label: "视频提供商", value: config.video.provider === "ai-douyin" ? "AI Douyin" : "TikHub" },
    { label: "输出目录", value: config.output.note_dir || "❌ 未设置", ok: !!config.output.note_dir },
    { label: "功能开启", value: Object.entries(config.features).filter(([, v]) => v).length + " 项" },
  ];

  return (
    <div>
      <p style={{ fontSize: 14, color: "var(--text-secondary, #a0a0c0)", marginBottom: 20, lineHeight: 1.6 }}>
        以下是你的配置摘要。确认无误后点击"完成并保存"，之后可在"设置"页随时调整。
      </p>

      <div style={{ display: "flex", flexDirection: "column", gap: 0, borderRadius: 8, overflow: "hidden", border: "1px solid var(--border, #2a2a4a)" }}>
        {rows.map((row, i) => (
          <div key={i} style={{
            display: "flex", justifyContent: "space-between", alignItems: "center",
            padding: "10px 16px", fontSize: 13,
            background: i % 2 === 0 ? "var(--bg-surface, #1a1a2e)" : "var(--bg-app, #0f0f1a)",
            borderBottom: i < rows.length - 1 ? "1px solid var(--border, #2a2a4a)" : "none",
          }}>
            <span style={{ color: "var(--text-muted, #666)" }}>{row.label}</span>
            <span style={{
              color: row.ok === false ? "#f87171" : row.ok === true ? "#4ade80" : "var(--text, #e0e0f0)",
              fontWeight: 500,
            }}>
              {row.value}
            </span>
          </div>
        ))}
      </div>

      {/* 完成后说明 */}
      <div style={{
        marginTop: 16, padding: "10px 14px", borderRadius: 8, fontSize: 12,
        background: "rgba(99,102,241,0.08)", border: "1px solid rgba(99,102,241,0.2)",
        color: "#a5b4fc",
      }}>
        保存后你可以立即开始炼化第一条笔记。后续在"设置"页可以随时调整模型、截图、输出目录和 API Key。
      </div>
    </div>
  );
}
