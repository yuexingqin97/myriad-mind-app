// ============================================================
// SettingsPage — 左-右双栏设置页
// 顶部健康度卡片 + 左侧导航 + 右侧内容
// ============================================================

import React, { useState, useEffect } from "react";
import type { MyriadMindConfig } from "@myriad-mind/core";
import { Input, Select, Toggle } from "./common/Input.js";
import { Button } from "./common/Button.js";
import type { DepsInfo, HealthStatus } from "./types.js";

export type ThemeMode = "light" | "dark" | "system";

export interface SettingsPageProps {
  config: MyriadMindConfig;
  onSave: (config: MyriadMindConfig) => void;
  keychain?: KeychainApi;
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
  /** 打开输出目录 */
  onOpenOutputDir?: () => void;
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
}

export interface KeychainApi {
  check(service: string): Promise<boolean>;
  read(service: string): Promise<string>;
  store(service: string, secret: string): Promise<void>;
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
  config: initialConfig,
  onSave,
  keychain,
  deps,
  onRecheckDeps,
  onOpenWizard,
  onResetConfig,
  onSelectOutputDir,
  onOpenOutputDir,
  configPath,
  theme,
  onThemeChange,
  appIcon,
  onTestAiConnection,
}: SettingsPageProps) {
  const [tab, setTab] = useState<TabId>("overview");
  const [config, setConfig] = useState<MyriadMindConfig>({ ...initialConfig });
  const [saved, setSaved] = useState(false);

  const update = <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => {
    setConfig((c) => ({ ...c, [key]: value }));
    setSaved(false);
  };

  const handleSave = () => {
    onSave(config);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  // ---- Health status derivation ----

  const healthItems = React.useMemo(() => {
    const items: Array<{ label: string; icon: string; status: HealthStatus; detail: string }> = [];

    // AI Key — check if any AI provider key is configured
    items.push({
      label: "AI 模型", icon: "🧠",
      status: keychain ? "ok" : "unconfigured",
      detail: keychain ? "已连接密钥链" : "开发模式",
    });

    // Python
    if (deps?.python) {
      items.push({
        label: "Python", icon: "🐍",
        status: deps.python.found ? "ok" : "error",
        detail: deps.python.version ?? (deps.python.found ? "就绪" : "未安装"),
      });
    } else {
      items.push({ label: "Python", icon: "🐍", status: "unconfigured", detail: "未检测" });
    }

    // FFmpeg
    if (deps?.ffmpeg) {
      items.push({
        label: "FFmpeg", icon: "🎞️",
        status: deps.ffmpeg.found ? "ok" : "warning",
        detail: deps.ffmpeg.version ?? (deps.ffmpeg.found ? "就绪" : "未安装"),
      });
    } else {
      items.push({ label: "FFmpeg", icon: "🎞️", status: "unconfigured", detail: "未检测" });
    }

    // 输出目录
    items.push({
      label: "输出目录", icon: "📂",
      status: config.output.note_dir ? "ok" : "unconfigured",
      detail: config.output.note_dir || "使用默认目录",
    });

    return items;
  }, [keychain, deps, config.output.note_dir]);

  const allOk = healthItems.every((h) => h.status === "ok");
  const hasError = healthItems.some((h) => h.status === "error");

  const healthStatusColor = (s: HealthStatus) =>
    s === "ok" ? "#4ade80" : s === "warning" ? "#facc15" : s === "error" ? "#f87171" : "var(--text-muted, #666)";

  return (
    <div className="settings-page-full">
      {/* ---- 配置健康度 ---- */}
      <div className="settings-health">
        <div className="settings-health-cards">
          {healthItems.map((h) => (
            <div key={h.label} className="settings-health-card" style={{
              borderColor: h.status === "ok" ? "rgba(74,222,128,0.3)" : h.status === "warning" ? "rgba(250,204,21,0.3)" : h.status === "error" ? "rgba(248,113,113,0.3)" : "rgba(42,42,74,0.5)",
            }}>
              <span style={{ fontSize: 20 }}>{h.icon}</span>
              <div>
                <span style={{ fontWeight: 600, fontSize: 13, color: "var(--text, #e0e0f0)" }}>{h.label}</span>
                <span style={{ marginLeft: 6, fontSize: 11, color: healthStatusColor(h.status) }}>
                  {h.status === "ok" ? "✅" : h.status === "warning" ? "⚠️" : h.status === "error" ? "❌" : "⚪"}
                </span>
              </div>
              <span style={{ fontSize: 11, color: "var(--text-muted, #666)", marginLeft: "auto" }}>{h.detail}</span>
            </div>
          ))}
        </div>
        <div style={{ fontSize: 12, marginTop: 10, padding: "0 4px", color: hasError ? "#f87171" : allOk ? "#4ade80" : "#facc15" }}>
          {allOk
            ? "✅ 全部核心能力已就绪，可处理文章、视频和本地文件。"
            : hasError
              ? "❌ 核心能力缺失 — 还缺 AI API Key 或 Python，暂时无法生成笔记。"
              : "⚠️ 部分能力可用 — 文章处理可用，视频处理可能受限。"}
        </div>
      </div>

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
          {tab === "overview" && <OverviewTab config={config} healthItems={healthItems} allOk={allOk} hasError={hasError} configPath={configPath} onRecheckDeps={onRecheckDeps} onOpenWizard={onOpenWizard} onResetConfig={onResetConfig} theme={theme} onThemeChange={onThemeChange} onTestAiConnection={onTestAiConnection} />}
          {tab === "keys" && <KeysTab keychain={keychain} />}
          {tab === "processing" && <ProcessingTab config={config} update={update} deps={deps} />}
          {tab === "output" && <OutputTab config={config} update={update} onSelectDir={onSelectOutputDir} onOpenDir={onOpenOutputDir} />}
          {tab === "features" && <FeaturesTab config={config} update={update} />}
          {tab === "advanced" && <AdvancedTab config={config} update={update} />}
          {tab === "about" && <AboutTab appIcon={appIcon} />}
        </div>
      </div>

      {/* 底部保存栏 */}
      <div className="settings-footer">
        <span className="settings-footer-hint">
          {saved ? "✅ 已保存" : "修改后点击保存"}
        </span>
        <Button onClick={handleSave}>
          {saved ? "✅ 已保存" : "保存设置"}
        </Button>
      </div>
    </div>
  );
}

// ============================================================
// SettingsSection helper
// ============================================================

function SettingsSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 28 }}>
      <h3 style={{
        fontSize: 13, fontWeight: 600, color: "var(--text-secondary, #a0a0c0)",
        textTransform: "uppercase", letterSpacing: "0.06em",
        marginBottom: 12, paddingBottom: 8,
        borderBottom: "1px solid var(--border, #2a2a4a)",
      }}>
        {title}
      </h3>
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>{children}</div>
    </div>
  );
}

// ============================================================
// 概览 Tab
// ============================================================

function OverviewTab({
  config, healthItems, allOk, hasError, configPath, onRecheckDeps, onOpenWizard, onResetConfig, theme, onThemeChange, onTestAiConnection,
}: {
  config: MyriadMindConfig;
  healthItems: Array<{ label: string; icon: string; status: HealthStatus; detail: string }>;
  allOk: boolean; hasError: boolean;
  configPath?: string;
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
        <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
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

      <SettingsSection title="配置状态">
        <div style={{ fontSize: 12, lineHeight: 1.8, color: "var(--text-secondary, #a0a0c0)" }}>
          {healthItems.map((h) => (
            <div key={h.label} style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 4 }}>
              <span style={{ fontSize: 14, width: 24, textAlign: "center" }}>{h.icon}</span>
              <span style={{ fontWeight: 500, width: 80 }}>{h.label}</span>
              <span style={{ color: h.status === "ok" ? "#4ade80" : h.status === "warning" ? "#facc15" : h.status === "error" ? "#f87171" : "var(--text-muted, #666)" }}>
                {h.status === "ok" ? "正常" : h.status === "warning" ? "警告" : h.status === "error" ? "阻塞" : "未配置"}
              </span>
              <span style={{ color: "var(--text-muted, #666)", marginLeft: 8, fontSize: 11 }}>{h.detail}</span>
            </div>
          ))}
          {configPath && (
            <div style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 4 }}>
              <span style={{ fontSize: 14, width: 24, textAlign: "center" }}>📁</span>
              <span style={{ fontWeight: 500, width: 80 }}>配置路径</span>
              <span style={{ color: "var(--text-muted, #666)", fontSize: 11 }}>{configPath}</span>
            </div>
          )}
        </div>
      </SettingsSection>

      <SettingsSection title="快捷操作">
        <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
          {onRecheckDeps && (
            <Button variant="secondary" onClick={onRecheckDeps}>🔄 重新检测依赖</Button>
          )}
          {onOpenWizard && (
            <Button variant="secondary" onClick={onOpenWizard}>🧭 打开配置向导</Button>
          )}
          {onResetConfig && (
            <Button variant="secondary" onClick={onResetConfig}>🔄 重置配置</Button>
          )}
          {onTestAiConnection && (
            <Button variant="secondary" onClick={handleTestConnection} disabled={aiTesting}>
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

const KEY_DEFS: Array<{ service: string; label: string; desc: string; placeholder: string; required: boolean }> = [
  { service: "claude-api-key",  label: "Claude API Key（可选）", desc: "Anthropic 控制台 → API Keys。v2 备用，当前主力为 DeepSeek", placeholder: "sk-ant-api03-...", required: false },
  { service: "ai-douyin-api-key", label: "AI Douyin API Key", desc: "抖音/B 站/小红书视频解析。aidouyin.com 注册获取",        placeholder: "输入 Key...",       required: false },
  { service: "tikhub-token",     label: "TikHub Token",       desc: "视频解析备用方案。tikhub.io 注册获取",                    placeholder: "输入 Token...",     required: false },
  { service: "volcengine-token", label: "火山引擎 Token",     desc: "云端 ASR 后端 Token",                                     placeholder: "输入 Token...",     required: false },
  { service: "deepseek-api-key",  label: "DeepSeek API Key",   desc: "AI 笔记生成主力模型。platform.deepseek.com → API Keys",      placeholder: "sk-...",            required: true },
];

function KeysTab({ keychain }: { keychain?: KeychainApi }) {
  return (
    <SettingsSection title="API 密钥管理">
      <p style={{ fontSize: 12, color: "var(--text-muted, #666)", marginBottom: 16, lineHeight: 1.6 }}>
        所有密钥存储在 OS 密钥链（Windows 凭据管理器），绝不明文落盘。DeepSeek API Key 为必填项。
      </p>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {KEY_DEFS.map((def) => (
          <KeyField key={def.service} {...def} keychain={keychain} />
        ))}
      </div>
    </SettingsSection>
  );
}

function KeyField({ service, label, desc, placeholder, required, keychain }: {
  service: string; label: string; desc: string; placeholder: string; required: boolean; keychain?: KeychainApi;
}) {
  const [status, setStatus] = useState<"loading" | "set" | "empty">("loading");
  const [masked, setMasked] = useState("");
  const [editing, setEditing] = useState(false);
  const [inputVal, setInputVal] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!keychain) { setStatus("empty"); return; }
    keychain.check(service).then((exists) => {
      if (exists) return keychain.read(service).then((s) => { setMasked(maskKey(s)); setStatus("set"); });
      setStatus("empty");
    }).catch(() => setStatus("empty"));
  }, [service, keychain]);

  const handleSave = async () => {
    if (!inputVal.trim()) return;
    setSaving(true);
    try {
      await keychain?.store(service, inputVal.trim());
      setMasked(maskKey(inputVal.trim()));
      setStatus("set"); setEditing(false); setInputVal("");
    } finally { setSaving(false); }
  };

  return (
    <div className={`settings-key-row${status === "set" ? " settings-key-set" : required ? " settings-key-required" : ""}`}>
      <div className="settings-key-header">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span className="settings-key-label">{label}</span>
          {required && <span className="settings-key-badge settings-key-badge-required">必填</span>}
          {status === "set" && <span className="settings-key-badge settings-key-badge-ok">已配置</span>}
          {status === "empty" && !required && <span className="settings-key-badge settings-key-badge-optional">可选</span>}
        </div>
        {!editing && (
          <button className="settings-key-edit-btn" onClick={() => setEditing(true)}>
            {status === "set" ? "编辑" : "配置"}
          </button>
        )}
      </div>
      <p className="settings-key-desc">{desc}</p>
      {!editing && status === "set" && (
        <code className="settings-key-masked">{masked}</code>
      )}
      {editing && (
        <div className="settings-key-edit">
          <input type="password" value={inputVal} onChange={(e) => setInputVal(e.target.value)}
            placeholder={placeholder} onKeyDown={(e) => e.key === "Enter" && handleSave()} className="settings-key-input" />
          <button className="settings-key-save-btn" onClick={handleSave} disabled={!inputVal.trim() || saving}>
            {saving ? "…" : "保存"}
          </button>
          <button className="settings-key-cancel-btn" onClick={() => { setEditing(false); setInputVal(""); }}>取消</button>
        </div>
      )}
    </div>
  );
}

function maskKey(s: string): string {
  if (!s || s.length <= 16) return "••••••••";
  return s.slice(0, 8) + "..." + s.slice(-4);
}

// ============================================================
// 处理能力 Tab (合并 ASR + 视频 + 依赖)
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
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))", gap: 6 }}>
            {[
              { label: "Python", dep: deps.python, critical: true },
              { label: "FFmpeg", dep: deps.ffmpeg, critical: false },
              { label: "yt-dlp", dep: deps.ytdlp, critical: false },
              { label: "GPU/CUDA", dep: deps.gpu, critical: false },
            ].map(({ label, dep, critical }) => (
              <div key={label} style={{
                display: "flex", alignItems: "center", gap: 6,
                padding: "6px 10px", borderRadius: 6, fontSize: 11,
                background: dep.found ? "rgba(74,222,128,0.08)" : critical ? "rgba(248,113,113,0.08)" : "rgba(250,204,21,0.08)",
                border: `1px solid ${dep.found ? "rgba(74,222,128,0.2)" : critical ? "rgba(248,113,113,0.2)" : "rgba(250,204,21,0.2)"}`,
                color: dep.found ? "#4ade80" : critical ? "#f87171" : "#facc15",
              }}>
                <span>{dep.found ? "✅" : critical ? "❌" : "⚠️"}</span>
                <span style={{ fontWeight: 500 }}>{label}</span>
                {dep.version && <span style={{ color: "var(--text-muted, #666)", fontSize: 10 }}>{dep.version}</span>}
              </div>
            ))}
          </div>
        </SettingsSection>
      )}

      {/* 语音识别 */}
      <SettingsSection title="语音识别">
        <div style={{ display: "flex", gap: 6, marginBottom: 12 }}>
          <PillButton active={config.asr.backend === "faster-whisper"} onClick={() => update("asr", {
            ...config.asr, backend: "faster-whisper",
            faster_whisper: config.asr.faster_whisper ?? { model_size: "small", device: "auto" },
          })}>
            faster-whisper（本地免费）
          </PillButton>
          <PillButton active={config.asr.backend === "volcengine"} onClick={() => update("asr", {
            ...config.asr, backend: "volcengine",
            volcengine: config.asr.volcengine ?? { token_keyring_id: "volcengine-token", appid: "" },
          })}>
            火山引擎（云端付费）
          </PillButton>
        </div>

        {config.asr.backend === "faster-whisper" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <Select label="模型大小" value={config.asr.faster_whisper?.model_size ?? "small"}
              options={[
                { value: "tiny", label: "tiny — 最小最快" }, { value: "base", label: "base — 基础" },
                { value: "small", label: "small — 推荐" }, { value: "medium", label: "medium — 准确" },
                { value: "large-v3", label: "large-v3 — 最准确（需大显存）" },
              ]}
              onChange={(e) => update("asr", { ...config.asr, faster_whisper: {
                model_size: e.target.value as "tiny" | "base" | "small" | "medium" | "large-v3",
                device: config.asr.faster_whisper?.device ?? "auto",
                compute_type: config.asr.faster_whisper?.compute_type,
              }})}
            />
            <div>
              <label style={{ fontSize: 12, color: "var(--text-muted, #666)", marginBottom: 4, display: "block" }}>运行设备</label>
              <div style={{ display: "flex", gap: 6 }}>
                {(["auto", "cpu", "cuda"] as const).map((d) => (
                  <PillButton key={d} active={(config.asr.faster_whisper?.device ?? "auto") === d}
                    onClick={() => update("asr", { ...config.asr, faster_whisper: {
                      model_size: config.asr.faster_whisper?.model_size ?? "small",
                      device: d, compute_type: config.asr.faster_whisper?.compute_type,
                    }})}>
                    {d === "auto" ? "自动检测" : d === "cpu" ? "CPU" : "CUDA (GPU)"}
                  </PillButton>
                ))}
              </div>
            </div>
          </div>
        )}

        {config.asr.backend === "volcengine" && (
          <Input
            label="App ID"
            value={config.asr.volcengine?.appid ?? ""}
            placeholder="火山引擎控制台 → 语音技术 → 应用 ID"
            hint="Token 请在「API 密钥」标签页配置"
            onChange={(e) => update("asr", { ...config.asr, volcengine: { token_keyring_id: "volcengine-token", appid: e.target.value } })}
          />
        )}
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
          抖音/小红书/B 站视频解析需要 API Key（在「API 密钥」标签页配置）。YouTube 不需要额外 Key。
        </p>
      </SettingsSection>
    </>
  );
}

// ============================================================
// 输出 Tab
// ============================================================

function OutputTab({
  config, update, onSelectDir, onOpenDir,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
  onSelectDir?: () => Promise<string | null>;
  onOpenDir?: () => void;
}) {
  return (
    <>
      <SettingsSection title="笔记输出">
        <Input
          label="笔记输出目录"
          value={config.output.note_dir}
          placeholder="留空则输出到默认目录。例: D:/Notes/ 或 ./大衍决残卷/"
          hint="绝对路径或相对路径。选在云盘文件夹可跨设备同步。"
          onChange={(e) => update("output", { ...config.output, note_dir: e.target.value })}
        />
        <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
          {onSelectDir && (
            <Button variant="secondary" onClick={async () => {
              const dir = await onSelectDir();
              if (dir) update("output", { ...config.output, note_dir: dir });
            }}>📁 选择目录</Button>
          )}
          {onOpenDir && (
            <Button variant="secondary" onClick={onOpenDir}>📂 打开目录</Button>
          )}
        </div>
      </SettingsSection>

      <SettingsSection title="文件管理">
        <Toggle label="自动清理临时文件"
          description="处理完成后删除 /tmp 中的视频、音频、字幕、截图"
          checked={config.output.cleanup_temp}
          onChange={(v) => update("output", { ...config.output, cleanup_temp: v })} />
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
        <Toggle label="关键帧截图" description="从视频截图并嵌入笔记"
          checked={config.features.keyframes} onChange={() => toggle("keyframes")} />
        {config.features.keyframes && (
          <div style={{ marginTop: 8 }}>
            <Input label="截图间隔（秒）" type="number" min={5} max={300}
              value={config.keyframes.interval}
              onChange={(e) => update("keyframes", { ...config.keyframes, interval: Math.max(5, Math.min(300, Number(e.target.value) || 30)) })} />
            <div style={{ marginTop: 8 }}>
              <Select label="截图模式"
                options={[
                  { value: "interval", label: "固定间隔" },
                  { value: "scene", label: "场景检测" },
                  { value: "both", label: "两者结合" },
                ]}
                value={config.keyframes.mode}
                onChange={(e) => update("keyframes", { ...config.keyframes, mode: e.target.value as "interval" | "scene" | "both" })} />
            </div>
          </div>
        )}
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
  config, update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
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
        {config.asr.backend === "faster-whisper" && (
          <Select
            label="compute_type"
            options={[
              { value: "default", label: "default — 自动" },
              { value: "float16", label: "float16" },
              { value: "int8", label: "int8 — 量化加速" },
            ]}
            value={config.asr.faster_whisper?.compute_type ?? "default"}
            onChange={(e) => update("asr", { ...config.asr, faster_whisper: {
              ...config.asr.faster_whisper!,
              compute_type: e.target.value as "default" | "float16" | "int8",
            }})}
          />
        )}
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
