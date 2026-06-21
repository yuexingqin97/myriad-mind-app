// ============================================================
// SettingsPage — 左-右双栏设置页
// 顶部健康度卡片 + 左侧导航 + 右侧内容
// 保存策略：修改后 debounce 自动保存（无手动按钮）
// ============================================================

import { useState, useEffect } from "react";
import type React from "react";
import type { MyriadMindConfig } from "@myriad-mind/core";
import { Input, Select, Toggle } from "./common/Input.js";
import { Button } from "./common/Button.js";
import type { DepsInfo } from "./types.js";

export type ThemeMode = "light" | "dark" | "system";

/** 日志级别（与 api.ts LogLevel 对齐；error 级别对用户无意义，不下放） */
export type LogLevel = "trace" | "debug" | "info" | "warn";

export interface SettingsPageProps {
  config: MyriadMindConfig;
  /** 配置字段变更（直接改 App 内存 config，由 useConfig debounce 自动写盘） */
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  /** 系统依赖状态 */
  deps?: DepsInfo;
  /** 重新检测依赖回调 */
  onRecheckDeps?: () => void;
  /** 打开配置向导 */
  onOpenWizard?: () => void;
  /** 重置配置 */
  onResetConfig?: () => void;
  /** 选择输出目录 */
  onSelectOutputDir?: () => Promise<string | null>;
  /** 打开输出目录（传目录路径，调系统文件管理器） */
  onOpenOutputDir?: (dir: string) => void;
  /** 打开缓存目录 */
  onOpenCacheDir?: () => void;
  /** config.json 路径 */
  configPath?: string;
  /** 当前主题 */
  theme?: ThemeMode;
  /** 主题切换回调 */
  onThemeChange?: (theme: ThemeMode) => void;
  /** App 图标 URL */
  appIcon?: string;
  /** 测试 AI 连接回调 */
  onTestAiConnection?: () => Promise<string>;
  /** 打开外部链接（注册页等，调系统浏览器） */
  onOpenUrl?: (url: string) => void;
  /** 当前日志级别（运行时偏好，由前端 localStorage 持久化，不入 config.json） */
  logLevel?: LogLevel;
  /** 日志级别切换回调（即时下发 Rust log::set_max_level） */
  onLogLevelChange?: (level: LogLevel) => void;
  /** 打开日志目录（~/.myriad-mind-app/logs/） */
  onOpenLogDir?: () => void;
}

type TabId = "overview" | "keys" | "processing" | "output" | "features" | "advanced" | "about";

const TABS: Array<{ id: TabId; label: string; icon: string }> = [
  { id: "overview",   label: "概览",      icon: "📋" },
  { id: "keys",       label: "API 密钥",  icon: "🔑" },
  { id: "processing", label: "处理能力",  icon: "⚙️" },
  { id: "output",     label: "输出",      icon: "📂" },
  { id: "features",   label: "笔记内容",  icon: "📝" },
  { id: "advanced",   label: "高级",      icon: "🔬" },
  { id: "about",      label: "关于",      icon: "ℹ️" },
];

// ============================================================

export function SettingsPage({
  config,
  update,
  deps,
  onRecheckDeps,
  onOpenWizard,
  onResetConfig,
  onSelectOutputDir,
  onOpenOutputDir,
  onOpenCacheDir,
  configPath,
  theme,
  onThemeChange,
  appIcon,
  onTestAiConnection,
  onOpenUrl,
  logLevel,
  onLogLevelChange,
  onOpenLogDir,
}: SettingsPageProps) {
  const [tab, setTab] = useState<TabId>("overview");
  // config / update 由父级 useConfig 提供：改动即时进 App 内存 config，
  // 由 useConfig 的 debounce effect 自动写盘 —— 切到向导/其他视图时不会丢失未保存的改动。

  return (
    <div className="settings-page-full" style={{ position: "relative" }}>
      {/* ---- 双栏布局 ---- */}
      <div className="settings-dual-layout">
        {/* 左侧导航 */}
        <nav className="settings-nav-left">
          {TABS.map((t) => (
            <button
              key={t.id}
              className={`settings-nav-item${tab === t.id ? " settings-nav-item-active" : ""}`}
              onClick={() => setTab(t.id)}
            >
              <span style={{ fontSize: 16, width: 24, textAlign: "center" }}>{t.icon}</span>
              <span>{t.label}</span>
            </button>
          ))}
        </nav>

        {/* 右侧内容 */}
        <div className="settings-content-right">
          {tab === "overview" && <OverviewTab onRecheckDeps={onRecheckDeps} onOpenWizard={onOpenWizard} onResetConfig={onResetConfig} theme={theme} onThemeChange={onThemeChange} onTestAiConnection={onTestAiConnection} />}
          {tab === "keys" && <KeysTab config={config} update={update} onOpenUrl={onOpenUrl} />}
          {tab === "processing" && <ProcessingTab config={config} update={update} deps={deps} />}
          {tab === "output" && <OutputTab config={config} update={update} onSelectDir={onSelectOutputDir} onOpenDir={onOpenOutputDir} onOpenCacheDir={onOpenCacheDir} />}
          {tab === "features" && <FeaturesTab config={config} update={update} />}
          {tab === "advanced" && <AdvancedTab config={config} update={update} logLevel={logLevel} onLogLevelChange={onLogLevelChange} onOpenLogDir={onOpenLogDir} />}
          {tab === "about" && <AboutTab appIcon={appIcon} />}
        </div>
      </div>

    </div>
  );
}

// ============================================================
// SettingsSection helper
// ============================================================

function SettingsSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 16 }}>
      <h3 style={{
        fontSize: 11, fontWeight: 600, color: "var(--text-secondary, #a0a0c0)",
        marginBottom: 6, paddingBottom: 4,
        borderBottom: "1px solid var(--border, #2a2a4a)",
      }}>
        {title}
      </h3>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>{children}</div>
    </div>
  );
}

// ============================================================
// 概览 Tab
// ============================================================

function OverviewTab({
  onRecheckDeps, onOpenWizard, onResetConfig, theme, onThemeChange, onTestAiConnection,
}: {
  onRecheckDeps?: () => void;
  onOpenWizard?: () => void;
  onResetConfig?: () => void;
  theme?: ThemeMode;
  onThemeChange?: (theme: ThemeMode) => void;
  onTestAiConnection?: () => Promise<string>;
}) {
  const [aiTestResult, setAiTestResult] = useState<string | null>(null);
  const [aiTesting, setAiTesting] = useState(false);

  const handleTestConnection = async () => {
    if (!onTestAiConnection) return;
    setAiTesting(true);
    setAiTestResult(null);
    try {
      const result = await onTestAiConnection();
      setAiTestResult(`✅ ${result}`);
    } catch (e) {
      setAiTestResult(`❌ ${e}`);
    } finally {
      setAiTesting(false);
    }
  };
  return (
    <>
      <SettingsSection title="外观主题">
        <p style={{ fontSize: 11, color: "var(--text-muted)", margin: "0 0 6px" }}>
          选择应用的外观主题，立即生效
        </p>
        <div className="settings-pill-group">
          {([
            { value: "light" as ThemeMode, label: "☀️ 浅色" },
            { value: "dark" as ThemeMode, label: "🌙 深色" },
            { value: "system" as ThemeMode, label: "💻 跟随系统" },
          ]).map((opt) => (
            <button
              key={opt.value}
              className={`settings-pill${theme === opt.value ? " settings-pill-active" : ""}`}
              onClick={() => onThemeChange?.(opt.value)}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </SettingsSection>

      <SettingsSection title="快捷操作">
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
          {onRecheckDeps && (
            <Button variant="secondary" size="sm" onClick={onRecheckDeps}>🔄 重新检测依赖</Button>
          )}
          {onOpenWizard && (
            <Button variant="secondary" size="sm" onClick={onOpenWizard}>🧭 打开配置向导</Button>
          )}
          {onResetConfig && (
            <Button variant="secondary" size="sm" onClick={onResetConfig}>🔄 重置配置</Button>
          )}
          {onTestAiConnection && (
            <Button variant="secondary" size="sm" onClick={handleTestConnection} disabled={aiTesting}>
              {aiTesting ? "⏳ 测试中…" : "🧪 测试 AI 连接"}
            </Button>
          )}
        </div>
        {aiTestResult && (
          <p style={{ fontSize: 12, marginTop: 8, color: aiTestResult.startsWith("✅") ? "#4ade80" : "#f87171" }}>
            {aiTestResult}
          </p>
        )}
      </SettingsSection>
    </>
  );
}

// ============================================================
// API 密钥 Tab
// ============================================================

function KeysTab({
  config, update, onOpenUrl,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  onOpenUrl?: (url: string) => void;
}) {
  return (
    <>
      <SettingsSection title="🤖 AI 模型">
        {/* Model selector */}
        <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
          {[
            { id: "deepseek", label: "DeepSeek", available: true },
            { id: "claude", label: "Claude", available: false },
          ].map((p) => (
            <div
              key={p.id}
              style={{
                flex: 1, padding: "8px 12px", borderRadius: 8,
                border: p.id === "deepseek" ? "2px solid #1683ff" : "1px solid var(--border, #2a2a4a)",
                background: p.id === "deepseek" ? "rgba(22,131,255,0.06)" : "var(--bg-surface, #1a1a2e)",
                opacity: p.available ? 1 : 0.4,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ fontWeight: 600, fontSize: 12, color: "var(--text, #e0e0f0)" }}>{p.label}</span>
                {p.id === "deepseek" && <span style={{ fontSize: 9, padding: "1px 5px", borderRadius: 3, background: "rgba(22,131,255,0.15)", color: "#1683ff" }}>当前</span>}
                {!p.available && <span style={{ fontSize: 9, color: "var(--text-muted, #666)" }}>即将支持</span>}
              </div>
              <p style={{ fontSize: 10, color: "var(--text-muted, #666)", margin: "2px 0 0" }}>
                {p.id === "deepseek" ? "V4 Pro / Flash · 1M 上下文" : "Anthropic · 后续版本"}
              </p>
            </div>
          ))}
        </div>

        <ConfigKeyInput label="DeepSeek API Key"
          desc="platform.deepseek.com → API Keys" placeholder="sk-..."
          value={config.deepseek_api_key ?? ""}
          onChange={(v) => update("deepseek_api_key", v)}
          link={{ url: "https://platform.deepseek.com/api_keys", label: "前往 DeepSeek 控制台 ↗" }}
          onOpenUrl={onOpenUrl} />
      </SettingsSection>

      <SettingsSection title="📡 视频解析服务">
        <ConfigKeyInput label="AI Douyin API Key"
          desc="B站/抖音/小红书视频解析。ai-douyin.top9.cc 注册获取" placeholder="输入 Key..."
          value={config.ai_douyin_api_key ?? ""}
          onChange={(v) => update("ai_douyin_api_key", v)}
          link={{ url: "https://ai-douyin.top9.cc/", label: "前往 ai-douyin.top9.cc ↗" }}
          onOpenUrl={onOpenUrl} />
      </SettingsSection>
    </>
  );
}

/** 受控 Key 输入框（修改后随设置页 debounce 自动保存到 ~/.myriad-mind-app/config.json） */
function ConfigKeyInput({
  label, desc, placeholder, value, onChange, link, onOpenUrl,
}: {
  label: string; desc: string; placeholder: string;
  value: string; onChange: (v: string) => void;
  link?: { url: string; label?: string };
  onOpenUrl?: (url: string) => void;
}) {
  const [show, setShow] = useState(false);
  return (
    <div style={{
      padding: "8px 12px", borderRadius: 6, marginBottom: 4,
      border: "1px solid var(--border, #2a2a4a)", background: value ? "rgba(74,222,128,0.04)" : "var(--bg-surface)",
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 4 }}>
        <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-secondary, #a0a0c0)" }}>
          {label} {value ? <span style={{ color: "#4ade80", fontSize: 10 }}>✅ 已配置</span> : null}
        </span>
        <button
          onClick={() => setShow(!show)}
          style={{
            fontSize: 10, padding: "2px 8px", borderRadius: 4, cursor: "pointer",
            border: "1px solid var(--border, #2a2a4a)", background: "var(--bg-app)",
            color: "var(--text-secondary, #a0a0c0)",
          }}
        >
          {show ? "隐藏" : value ? "编辑" : "配置"}
        </button>
      </div>
      <p style={{ fontSize: 10, color: "var(--text-muted, #666)", margin: "0 0 4px" }}>{desc}</p>
      {link && (
        <a
          href={link.url} target="_blank" rel="noopener noreferrer"
          onClick={onOpenUrl ? (e) => { e.preventDefault(); onOpenUrl(link.url); } : undefined}
          style={{ fontSize: 10, color: "var(--brand-primary, #1683ff)", textDecoration: "none", display: "inline-block", marginTop: 2 }}
        >
          {link.label ?? link.url}
        </a>
      )}
      {show && (
        <div style={{ display: "flex", gap: 6 }}>
          <input
            type="password" value={value} onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            style={{
              flex: 1, padding: "5px 8px", fontSize: 11, borderRadius: 4,
              border: "1px solid var(--border, #333)", background: "var(--bg-input, #1f2026)",
              color: "var(--text, #e0e0f0)", outline: "none",
            }}
          />
        </div>
      )}
      {show && !value && (
        <p style={{ fontSize: 9, color: "#facc15", margin: "4px 0 0" }}>
          ⚠️ 输入后自动保存到 ~/.myriad-mind-app/config.json（明文，请自行保管文件安全）
        </p>
      )}
    </div>
  );
}

// ============================================================
// 处理能力 Tab (ASR + 视频 + 依赖)
// ============================================================

function ProcessingTab({
  config, update, deps,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  deps?: DepsInfo;
}) {
  return (
    <>
      {/* 本机依赖 */}
      {deps && (
        <SettingsSection title="本机依赖">
          {([
            { label: "Python", icon: "🐍", dep: deps.python, critical: true,
              usage: "ASR 转写、脚本调度 — 整个管线引擎依赖 Python 环境",
              fix: "安装 Python 3.9+ → python.org 或 winget install Python.Python.3.12" },
            { label: "FFmpeg", icon: "🎞️", dep: deps.ffmpeg, critical: false,
              usage: "视频解码、音频提取、关键帧截图 — 处理视频必需",
              fix: "winget install FFmpeg 或 ffmpeg.org 下载" },
            { label: "faster-whisper", icon: "🎙️", dep: deps.fasterWhisper, critical: true,
              usage: "本地 ASR 转写 — 没有可用字幕时必须回退到它生成字幕",
              fix: "使用应用检测到的 Python 执行：python -m pip install -U faster-whisper" },
            { label: "yt-dlp", icon: "⬇️", dep: deps.ytdlp, critical: false,
              usage: "下载在线视频、提取字幕 — YouTube/B 站等平台依赖",
              fix: "winget install yt-dlp.yt-dlp 或 pip install yt-dlp" },
            { label: "GPU/CUDA", icon: "🖥️", dep: deps.gpu, critical: false,
              usage: "加速本地 ASR — 无 GPU 时自动降级为 CPU 模式",
              fix: "可选安装 CUDA Toolkit，CPU 模式也可正常使用" },
          ] as const).map(({ label, icon, dep, critical, usage, fix }) => (
            <div key={label} style={{
              padding: "10px 14px", borderRadius: 8, marginBottom: 8,
              border: `1px solid ${dep.found ? "rgba(74,222,128,0.2)" : critical ? "rgba(248,113,113,0.2)" : "rgba(250,204,21,0.2)"}`,
              background: dep.found ? "rgba(74,222,128,0.04)" : critical ? "rgba(248,113,113,0.04)" : "rgba(250,204,21,0.04)",
            }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                <span style={{ fontSize: 16 }}>{icon}</span>
                <span style={{ fontWeight: 600, fontSize: 13, color: dep.found ? "#4ade80" : critical ? "#f87171" : "#facc15" }}>
                  {label} {dep.found ? "✅" : critical ? "❌" : "⚠️"}
                </span>
                {dep.version && <span style={{ fontSize: 11, color: "var(--text-muted, #666)" }}>{dep.version}</span>}
              </div>
              <p style={{ fontSize: 11, color: "var(--text-secondary, #a0a0c0)", margin: 0, lineHeight: 1.5 }}>{usage}</p>
              {!dep.found && <p style={{ fontSize: 11, color: critical ? "#f87171" : "#facc15", margin: "4px 0 0", lineHeight: 1.4 }}>💡 {fix}</p>}
            </div>
          ))}
        </SettingsSection>
      )}

      {/* 语音识别（固定 faster-whisper 本地转写；云端 ASR 暂未支持） */}
      <SettingsSection title="语音识别">
        <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "0 0 10px" }}>
          使用 faster-whisper 本地转写（免费、离线）。云端 ASR（火山引擎）暂未接入。
        </p>
        <Select label="模型大小" value={config.asr.faster_whisper?.model_size ?? "small"}
          options={[
            { value: "tiny", label: "tiny — 最小最快" }, { value: "base", label: "base — 基础" },
            { value: "small", label: "small — 推荐" }, { value: "medium", label: "medium — 准确" },
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
        <div>
          <label style={{ fontSize: 12, color: "var(--text-muted, #666)", marginBottom: 4, display: "block" }}>运行设备</label>
          <div style={{ display: "flex", gap: 6 }}>
            {(["auto", "cpu", "cuda"] as const).map((d) => (
              <PillButton key={d} active={(config.asr.faster_whisper?.device ?? "auto") === d}
                onClick={() => update("asr", {
                  backend: "faster-whisper",
                  faster_whisper: {
                    model_size: config.asr.faster_whisper?.model_size ?? "small",
                    device: d, compute_type: config.asr.faster_whisper?.compute_type,
                  },
                })}>
                {d === "auto" ? "自动检测" : d === "cpu" ? "CPU" : "CUDA (GPU)"}
              </PillButton>
            ))}
          </div>
        </div>
      </SettingsSection>

      {/* 视频解析 */}
      <SettingsSection title="视频解析">
        <div style={{ display: "flex", gap: 6, marginBottom: 8 }}>
          <PillButton active={config.video.provider === "ai-douyin"} onClick={() => update("video", { provider: "ai-douyin" })}>
            AI Douyin（推荐）
          </PillButton>
          <PillButton active={config.video.provider === "tikhub"} onClick={() => update("video", { provider: "tikhub" })}>
            TikHub
          </PillButton>
        </div>
        <p style={{ fontSize: 11, color: "var(--text-muted, #666)" }}>
          抖音/小红书/B 站视频解析需要 AI Douyin API Key（在「API 密钥」标签页配置）。YouTube 不需要额外 Key。
        </p>
      </SettingsSection>
    </>
  );
}

// ============================================================
// 输出 Tab
// ============================================================

function OutputTab({
  config, update, onSelectDir, onOpenDir, onOpenCacheDir,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  onSelectDir?: () => Promise<string | null>;
  onOpenDir?: (dir: string) => void;
  onOpenCacheDir?: () => void;
}) {
  return (
    <>
      <SettingsSection title="笔记输出">
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
            <Button variant="secondary" onClick={async () => {
              const dir = await onSelectDir();
              if (dir) update("output", { ...config.output, note_dir: dir });
            }}>📁 选择目录</Button>
          )}
        </div>
        <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "4px 0 0" }}>
          笔记将保存到此目录。选在云盘文件夹可跨设备同步
        </p>
        {!config.output.note_dir && (
          <p style={{ fontSize: 11, color: "#f87171", marginTop: 4 }}>⚠️ 必须设置输出目录，否则无法保存生成的笔记</p>
        )}
      </SettingsSection>

      <SettingsSection title="文件管理">
        <Toggle label="自动清理临时文件"
          description="处理完成后删除 /tmp 中的视频、音频、字幕、截图。关闭此项可保留缓存视频（调试用）"
          checked={config.output.cleanup_temp}
          onChange={(v) => update("output", { ...config.output, cleanup_temp: v })} />
        {onOpenCacheDir && (
          <div style={{ marginTop: 10 }}>
            <Button variant="secondary" onClick={onOpenCacheDir}>
              📁 打开缓存目录
            </Button>
            <span style={{ fontSize: 11, color: "var(--text-muted, #666)", marginLeft: 10 }}>
              查看处理过程中产生的临时文件（视频、音频、字幕、截图）
            </span>
          </div>
        )}
      </SettingsSection>

      <SettingsSection title="笔记元信息">
        <Toggle label="笔记末尾添加元信息"
          description="记录生成时间、模型、Token 消耗等"
          checked={config.output.note_metadata}
          onChange={(v) => update("output", { ...config.output, note_metadata: v })} />
        <Toggle label="调试元信息"
          description="额外输出处理链路详情（决策链路、各步骤消耗）"
          checked={config.output.debug_metadata}
          onChange={(v) => update("output", { ...config.output, debug_metadata: v })} />
      </SettingsSection>
    </>
  );
}

// ============================================================
// 笔记内容 Tab
// ============================================================

function FeaturesTab({
  config, update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
}) {
  const toggle = (key: keyof typeof config.features) => {
    update("features", { ...config.features, [key]: !config.features[key] });
  };

  return (
    <>
      <SettingsSection title="笔记内容">
        <Toggle label="Mermaid 图表" description="自动绘制架构图、流程图、时序图"
          checked={config.features.mermaid} onChange={() => toggle("mermaid")} />
        <Toggle label="扩展学习资源" description="推荐相关文档、视频、GitHub 仓库等"
          checked={config.features.resources} onChange={() => toggle("resources")} />
        <Toggle label="评论区精华" description="自动提取视频评论区高质量讨论"
          checked={config.features.comments} onChange={() => toggle("comments")} />
      </SettingsSection>

      <SettingsSection title="视频处理">
        <Toggle label="关键帧截图" description="AI 字幕分析推荐时间点 + 场景变化检测"
          checked={config.features.keyframes} onChange={() => toggle("keyframes")} />
        {config.features.keyframes && (
          <div style={{ marginTop: 6 }}>
            {/* ---- AI 截图审查 ---- */}
            <div style={{ padding: "10px 12px", borderRadius: 6,
              border: "1px solid rgba(22,131,255,0.2)", background: "rgba(22,131,255,0.04)" }}>
              <Toggle label="🤖 AI 智能截图审查"
                description="使用 DeepSeek Vision 逐张分析截图价值，自动过滤纯人脸、黑屏、过渡画面"
                checked={config.features.screenshot_review?.enabled ?? true}
                onChange={(v) => update("features", {
                  ...config.features,
                  screenshot_review: { ...config.features.screenshot_review!, enabled: v },
                })} />
              {config.features.screenshot_review?.enabled && (
                <div style={{ display: "flex", flexDirection: "column", gap: 8, marginTop: 10 }}>
                  <Select label="审查模式"
                    options={[
                      { value: "hybrid", label: "混合模式 — 先批量粗筛再逐张精审（推荐）" },
                      { value: "batch", label: "批量模式 — 一次提交全部截图" },
                      { value: "single", label: "逐张模式 — 最高精度，较慢" },
                    ]}
                    value={config.features.screenshot_review.mode}
                    onChange={(e) => update("features", {
                      ...config.features,
                      screenshot_review: {
                        ...config.features.screenshot_review!,
                        mode: e.target.value as "hybrid" | "batch" | "single",
                      },
                    })} />
                  <div style={{ display: "flex", gap: 12 }}>
                    <div style={{ flex: 1 }}>
                      <Input label="最低入选分数 (0-3)"
                        type="number" min={0} max={3}
                        value={config.features.screenshot_review.min_score}
                        onChange={(e) => update("features", {
                          ...config.features,
                          screenshot_review: {
                            ...config.features.screenshot_review!,
                            min_score: Math.max(0, Math.min(3, Number(e.target.value) || 2)),
                          },
                        })} />
                    </div>
                    <div style={{ flex: 1 }}>
                      <Input label="最多入选张数 (3-20)"
                        type="number" min={3} max={20}
                        value={config.features.screenshot_review.max_selected}
                        onChange={(e) => update("features", {
                          ...config.features,
                          screenshot_review: {
                            ...config.features.screenshot_review!,
                            max_selected: Math.max(3, Math.min(20, Number(e.target.value) || 15)),
                          },
                        })} />
                    </div>
                  </div>
                </div>
              )}
              <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "8px 0 0" }}>
                每张截图约 81 tokens，一个视频 25 张候选截图全部审查约 0.015 元
              </p>
            </div>
          </div>
        )}

        <Toggle label="📋 教程模式检测"
          description="自动识别操作型教程（如 IDE 配置、软件操作），生成操作流程总览图"
          checked={config.features.tutorial_detection}
          onChange={() => toggle("tutorial_detection")} />
      </SettingsSection>

      <SettingsSection title="笔记信息">
        <Toggle label="阅读时长与难度评级" description="在笔记开头标注推荐阅读时间和内容难度"
          checked={config.features.reading_info} onChange={() => toggle("reading_info")} />
        <Toggle label="灵力预估" description="处理前显示 Token/时间/费用预估"
          checked={config.features.estimation} onChange={() => toggle("estimation")} />
      </SettingsSection>
    </>
  );
}

// ============================================================
// 高级 Tab
// ============================================================

function AdvancedTab({
  config, update, logLevel, onLogLevelChange, onOpenLogDir,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  logLevel?: LogLevel;
  onLogLevelChange?: (level: LogLevel) => void;
  onOpenLogDir?: () => void;
}) {
  return (
    <>
      <SettingsSection title="Python 环境">
        <Input
          label="Python 解释器路径"
          value={config.python_path ?? ""}
          placeholder="留空 = 自动探测系统 Python"
          hint="指定安装了 faster-whisper 的 Python venv 路径"
          onChange={(e) => update("python_path", e.target.value)}
        />
      </SettingsSection>

      <SettingsSection title="ASR 高级参数">
        <Select
          label="compute_type"
          options={[
            { value: "default", label: "default — 自动" },
            { value: "float16", label: "float16" },
            { value: "int8", label: "int8 — 量化加速" },
          ]}
          value={config.asr.faster_whisper?.compute_type ?? "default"}
          onChange={(e) => update("asr", {
            backend: "faster-whisper",
            faster_whisper: {
              model_size: config.asr.faster_whisper?.model_size ?? "small",
              device: config.asr.faster_whisper?.device ?? "auto",
              compute_type: e.target.value as "default" | "float16" | "int8",
            },
          })}
        />
      </SettingsSection>

      <SettingsSection title="收尾操作">
        <Toggle label="自动更新修为面板"
          description="每次生成笔记后自动刷新统计数据和成就进度"
          checked={config.post_process.auto_update_panel}
          onChange={(v) => update("post_process", { ...config.post_process, auto_update_panel: v })} />
        <Toggle label="学习路线推荐"
          description="基于当前知识结构推荐下一步学习方向"
          checked={config.post_process.auto_suggest_next}
          onChange={(v) => update("post_process", { ...config.post_process, auto_suggest_next: v })} />
      </SettingsSection>

      <SettingsSection title="日志与调试">
        <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "0 0 6px" }}>
          控制 Rust 后端日志输出级别（Stdout / 日志文件 / 前端 F12 console 三通道同步生效）。切换即时生效，不写入 config.json。
        </p>
        {onLogLevelChange && (
          <Select
            label="日志级别"
            value={logLevel ?? "info"}
            options={[
              { value: "trace", label: "Trace — 全量（调试，日志量大）" },
              { value: "debug", label: "Debug — 详细（开发排查）" },
              { value: "info", label: "Info — 默认（用户级）" },
              { value: "warn", label: "Warn — 仅警告与错误" },
            ]}
            onChange={(e) => onLogLevelChange(e.target.value as LogLevel)}
          />
        )}
        {onOpenLogDir && (
          <div style={{ marginTop: 10 }}>
            <Button variant="secondary" onClick={onOpenLogDir}>
              📁 打开日志目录
            </Button>
            <span style={{ fontSize: 11, color: "var(--text-muted, #666)", marginLeft: 10 }}>
              日志文件位于 ~/.myriad-mind-app/logs/（按 50MB 轮转，保留最近 2 份）
            </span>
          </div>
        )}
      </SettingsSection>
    </>
  );
}

// ============================================================
// 关于 Tab
// ============================================================

function AboutTab({ appIcon }: { appIcon?: string }) {
  return (
    <>
      <SettingsSection title="大衍决">
        <div style={{ textAlign: "center", padding: "20px 0" }}>
          {appIcon ? (
            <img src={appIcon} alt="大衍决" style={{ width: 64, height: 64, borderRadius: 14, objectFit: "cover" }} />
          ) : (
            <span style={{ fontSize: 48 }}>🧘</span>
          )}
          <h2 style={{ fontSize: 20, fontWeight: 700, margin: "8px 0 4px", color: "var(--text, #e0e0f0)" }}>大衍决</h2>
          <p style={{ fontSize: 12, color: "var(--text-muted, #666)", marginBottom: 4 }}>Myriad Mind v0.1.0</p>
          <p style={{ fontSize: 12, color: "var(--text-secondary, #a0a0c0)" }}>神识一扫，万物皆可为笔记</p>
        </div>
      </SettingsSection>

      <SettingsSection title="技术栈">
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
          {[
            ["前端", "React 19 + TypeScript"],
            ["后端", "Rust (Tauri 2.x)"],
            ["AI", "DeepSeek V4 (API)"],
            ["视频", "Python + FFmpeg + yt-dlp"],
            ["ASR", "faster-whisper"],
            ["存储", "Markdown + SQLite"],
          ].map(([k, v]) => (
            <div key={k} style={{ padding: "8px 12px", background: "var(--bg-app, #111)", borderRadius: 6, border: "1px solid var(--border, #2a2a4a)" }}>
              <p style={{ fontSize: 10, color: "var(--text-muted, #666)", margin: "0 0 2px" }}>{k}</p>
              <p style={{ fontSize: 12, color: "var(--text-secondary, #a0a0c0)", margin: 0 }}>{v}</p>
            </div>
          ))}
        </div>
      </SettingsSection>

      <SettingsSection title="项目与联系">
        <div style={{ fontSize: 12, color: "var(--text-secondary, #a0a0c0)", lineHeight: 2 }}>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <span style={{ fontSize: 14 }}>📦</span>
            <span>GitHub：</span>
            <a href="https://github.com/yuexingqin97/myriad-mind-app" target="_blank" rel="noopener noreferrer"
              style={{ color: "var(--brand-primary, #1683ff)", textDecoration: "none" }}>
              github.com/yuexingqin97/myriad-mind-app
            </a>
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <span style={{ fontSize: 14 }}>💬</span>
            <span>QQ：2410030025</span>
          </div>
        </div>
      </SettingsSection>

      <SettingsSection title="开源协议">
        <p style={{ fontSize: 12, color: "var(--text-secondary, #a0a0c0)" }}>
          MIT License — 免费开源，保留上游版权声明。
        </p>
      </SettingsSection>

      <SettingsSection title="⚠️ 免责声明">
        <div style={{
          fontSize: 12, color: "var(--text-secondary, #a0a0c0)", lineHeight: 1.8,
          padding: "14px 16px", borderRadius: 8,
          background: "rgba(250,204,21,0.05)", border: "1px solid rgba(250,204,21,0.15)",
        }}>
          <p style={{ margin: "0 0 8px", fontWeight: 500, color: "#facc15" }}>
            本工具仅供个人学习、研究、教育目的使用。
          </p>
          <ul style={{ margin: 0, paddingLeft: 18 }}>
            <li>本工具不存储、不托管、不传播任何视频或音频内容</li>
            <li>所有处理均在用户本地计算机上完成</li>
            <li>用户应遵守各内容平台的用户协议</li>
            <li>使用本工具下载受版权保护的内容需自行承担法律责任</li>
            <li>开发者不对用户的任何使用行为承担责任</li>
            <li>本工具不提供任何 API Key，所有第三方服务凭据由用户自行获取</li>
          </ul>
          <p style={{ margin: "8px 0 0", fontSize: 11, color: "var(--text-muted, #666)" }}>
            如果你是有版权的内容创作者，认为本工具侵犯了你的权益，请联系我们。
          </p>
        </div>
      </SettingsSection>
    </>
  );
}

// ---- Pill Button (用于 ASR/Video 切换) ----

function PillButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button className={`settings-pill${active ? " settings-pill-active" : ""}`} onClick={onClick}>
      {children}
    </button>
  );
}
