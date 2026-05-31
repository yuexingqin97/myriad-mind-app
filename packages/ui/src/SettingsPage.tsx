// ============================================================
// SettingsPage — CC Switch 风格设置页
// 水平 Tab 导航 + 分组设置项，非向导式
// ============================================================

import React, { useState } from "react";
import type { MyriadMindConfig } from "@myriad-mind/core";
import { Input, Select, Toggle } from "./common/Input.js";
import { Button } from "./common/Button.js";

export interface SettingsPageProps {
  config: MyriadMindConfig;
  onSave: (config: MyriadMindConfig) => void;
  keychain?: KeychainApi;
}

export interface KeychainApi {
  check(service: string): Promise<boolean>;
  read(service: string): Promise<string>;
  store(service: string, secret: string): Promise<void>;
}

type TabId = "general" | "asr" | "video" | "features" | "keys" | "output" | "about";

const TABS: Array<{ id: TabId; label: string }> = [
  { id: "general", label: "通用" },
  { id: "asr", label: "语音识别" },
  { id: "video", label: "视频解析" },
  { id: "features", label: "功能开关" },
  { id: "keys", label: "API 密钥" },
  { id: "output", label: "输出" },
  { id: "about", label: "关于" },
];

export function SettingsPage({ config: initialConfig, onSave, keychain }: SettingsPageProps) {
  const [tab, setTab] = useState<TabId>("general");
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

  return (
    <div className="settings-page">
      {/* 水平 Tab 导航 */}
      <div className="settings-tabs">
        {TABS.map((t) => (
          <button
            key={t.id}
            className={`settings-tab${tab === t.id ? " settings-tab-active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* 内容区 */}
      <div className="settings-content">
        {tab === "general" && <GeneralTab config={config} update={update} />}
        {tab === "asr" && <ASRTab config={config} update={update} />}
        {tab === "video" && <VideoTab config={config} update={update} />}
        {tab === "features" && <FeaturesTab config={config} update={update} />}
        {tab === "keys" && <KeysTab keychain={keychain} />}
        {tab === "output" && <OutputTab config={config} update={update} />}
        {tab === "about" && <AboutTab />}
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
// 各 Tab 内容
// ============================================================

function SettingsSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="settings-section">
      <h3 className="settings-section-title">{title}</h3>
      <div className="settings-section-body">{children}</div>
    </section>
  );
}

// ---- 通用 ----

function GeneralTab({ config, update }: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
}) {
  return (
    <>
      <SettingsSection title="Python 路径">
        <Input
          label="Python 解释器路径"
          value={config.python_path ?? ""}
          placeholder="留空 = 自动探测系统 Python"
          hint="指定安装了 faster-whisper 的 Python，如 C:/Users/xxx/.cache/myriad-mind/faster-whisper-venv/Scripts/python.exe"
          onChange={(e) => update("python_path", e.target.value)}
        />
      </SettingsSection>

      <SettingsSection title="收尾操作">
        <Toggle
          label="自动更新修为面板"
          description="每次生成笔记后自动刷新统计数据和成就进度"
          checked={config.post_process.auto_update_panel}
          onChange={(v) => update("post_process", { ...config.post_process, auto_update_panel: v })}
        />
        <Toggle
          label="学习路线推荐"
          description="基于当前知识结构推荐下一步学习方向"
          checked={config.post_process.auto_suggest_next}
          onChange={(v) => update("post_process", { ...config.post_process, auto_suggest_next: v })}
        />
      </SettingsSection>
    </>
  );
}

// ---- 语音识别 ----

function ASRTab({ config, update }: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
}) {
  return (
    <>
      <SettingsSection title="ASR 后端">
        <div className="settings-pill-group">
          <PillButton
            active={config.asr.backend === "faster-whisper"}
            onClick={() => update("asr", {
              ...config.asr,
              backend: "faster-whisper",
              faster_whisper: config.asr.faster_whisper ?? { model_size: "medium", device: "auto" },
            })}
          >
            faster-whisper（本地免费）
          </PillButton>
          <PillButton
            active={config.asr.backend === "volcengine"}
            onClick={() => update("asr", {
              ...config.asr,
              backend: "volcengine",
              volcengine: config.asr.volcengine ?? { token_keyring_id: "volcengine-token", appid: "" },
            })}
          >
            火山引擎（云端付费）
          </PillButton>
        </div>
      </SettingsSection>

      {config.asr.backend === "faster-whisper" && (
        <>
          <SettingsSection title="模型">
            <Select
              label="模型大小"
              options={[
                { value: "tiny", label: "tiny — 最小最快" },
                { value: "base", label: "base — 基础" },
                { value: "small", label: "small — 推荐" },
                { value: "medium", label: "medium — 准确" },
                { value: "large-v3", label: "large-v3 — 最准确（需大显存）" },
              ]}
              value={config.asr.faster_whisper?.model_size ?? "medium"}
              onChange={(e) => update("asr", {
                ...config.asr,
                faster_whisper: {
                  model_size: e.target.value as "tiny" | "base" | "small" | "medium" | "large-v3",
                  device: config.asr.faster_whisper?.device ?? "auto",
                  compute_type: config.asr.faster_whisper?.compute_type,
                },
              })}
            />
          </SettingsSection>

          <SettingsSection title="运行设备">
            <div className="settings-pill-group">
              {(["auto", "cpu", "cuda"] as const).map((d) => (
                <PillButton
                  key={d}
                  active={(config.asr.faster_whisper?.device ?? "auto") === d}
                  onClick={() => update("asr", {
                    ...config.asr,
                    faster_whisper: {
                      model_size: config.asr.faster_whisper?.model_size ?? "medium",
                      device: d,
                      compute_type: config.asr.faster_whisper?.compute_type,
                    },
                  })}
                >
                  {d === "auto" ? "自动检测" : d === "cpu" ? "CPU" : "CUDA (NVIDIA GPU)"}
                </PillButton>
              ))}
            </div>
          </SettingsSection>
        </>
      )}

      {config.asr.backend === "volcengine" && (
        <SettingsSection title="火山引擎配置">
          <Input
            label="App ID"
            value={config.asr.volcengine?.appid ?? ""}
            placeholder="火山引擎控制台 → 语音技术 → 应用 ID"
            hint="Token 请在「API 密钥」标签页配置"
            onChange={(e) => update("asr", {
              ...config.asr,
              volcengine: { token_keyring_id: "volcengine-token", appid: e.target.value },
            })}
          />
        </SettingsSection>
      )}
    </>
  );
}

// ---- 视频解析 ----

function VideoTab({ config, update }: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
}) {
  return (
    <SettingsSection title="视频解析提供商">
      <div className="settings-pill-group">
        <PillButton
          active={config.video.provider === "ai-douyin"}
          onClick={() => update("video", { provider: "ai-douyin" })}
        >
          AI Douyin（推荐）
        </PillButton>
        <PillButton
          active={config.video.provider === "tikhub"}
          onClick={() => update("video", { provider: "tikhub" })}
        >
          TikHub
        </PillButton>
      </div>
      <p className="settings-hint">
        抖音 / 小红书 / B 站视频解析需要 API Key（在「API 密钥」标签页配置）。YouTube 不需要额外 Key。
      </p>
    </SettingsSection>
  );
}

// ---- 功能开关 ----

function FeaturesTab({ config, update }: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
}) {
  const toggle = (key: keyof typeof config.features) => {
    update("features", { ...config.features, [key]: !config.features[key] });
  };

  return (
    <>
      <SettingsSection title="笔记内容">
        <Toggle label="Mermaid 图表" description="自动绘制架构图、流程图、时序图" checked={config.features.mermaid} onChange={() => toggle("mermaid")} />
        <Toggle label="扩展学习资源" description="推荐相关文档、视频、GitHub 仓库等" checked={config.features.resources} onChange={() => toggle("resources")} />
        <Toggle label="评论区精华" description="自动提取视频评论区高质量讨论" checked={config.features.comments} onChange={() => toggle("comments")} />
      </SettingsSection>

      <SettingsSection title="视频处理">
        <Toggle label="关键帧截图" description="从视频截图并嵌入笔记" checked={config.features.keyframes} onChange={() => toggle("keyframes")} />
        {config.features.keyframes && (
          <div style={{ marginTop: 8 }}>
            <Input
              label="截图间隔（秒）"
              type="number"
              min={5}
              max={300}
              value={config.keyframes.interval}
              onChange={(e) => update("keyframes", {
                ...config.keyframes,
                interval: Math.max(5, Math.min(300, Number(e.target.value) || 30)),
              })}
            />
            <div style={{ marginTop: 8 }}>
              <Select
                label="截图模式"
                options={[
                  { value: "interval", label: "固定间隔" },
                  { value: "scene", label: "场景检测" },
                  { value: "both", label: "两者结合" },
                ]}
                value={config.keyframes.mode}
                onChange={(e) => update("keyframes", {
                  ...config.keyframes,
                  mode: e.target.value as "interval" | "scene" | "both",
                })}
              />
            </div>
          </div>
        )}
      </SettingsSection>

      <SettingsSection title="笔记信息">
        <Toggle label="阅读时长与难度评级" description="在笔记开头标注推荐阅读时间和内容难度" checked={config.features.reading_info} onChange={() => toggle("reading_info")} />
        <Toggle label="灵力预估" description="处理前显示 Token / 时间 / 费用预估" checked={config.features.estimation} onChange={() => toggle("estimation")} />
      </SettingsSection>
    </>
  );
}

// ---- API 密钥 ----

const KEY_DEFS: Array<{
  service: string;
  label: string;
  desc: string;
  placeholder: string;
  required: boolean;
}> = [
  { service: "claude-api-key", label: "Claude API Key", desc: "Anthropic 控制台 → API Keys。格式: sk-ant-api03-...", placeholder: "sk-ant-api03-...", required: true },
  { service: "ai-douyin-api-key", label: "AI Douyin API Key", desc: "抖音/B 站/小红书视频解析。aidouyin.com 注册获取", placeholder: "输入 Key...", required: false },
  { service: "tikhub-token", label: "TikHub Token", desc: "视频解析备用方案。tikhub.io 注册获取", placeholder: "输入 Token...", required: false },
  { service: "volcengine-token", label: "火山引擎 Token", desc: "云端 ASR 后端 Token", placeholder: "输入 Token...", required: false },
];

function KeysTab({ keychain }: { keychain?: KeychainApi }) {
  return (
    <>
      <SettingsSection title="API 密钥管理">
        <p className="settings-hint" style={{ marginBottom: 16 }}>
          所有密钥存储在 OS 密钥链（Windows 凭据管理器），绝不明文落盘。Claude API Key 为必填项。
        </p>
        {KEY_DEFS.map((def) => (
          <KeyField key={def.service} {...def} keychain={keychain} />
        ))}
      </SettingsSection>
    </>
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

  React.useEffect(() => {
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
      setStatus("set");
      setEditing(false);
      setInputVal("");
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
          <input
            type="password"
            value={inputVal}
            onChange={(e) => setInputVal(e.target.value)}
            placeholder={placeholder}
            onKeyDown={(e) => e.key === "Enter" && handleSave()}
            className="settings-key-input"
          />
          <button className="settings-key-save-btn" onClick={handleSave} disabled={!inputVal.trim() || saving}>
            {saving ? "…" : "保存"}
          </button>
          <button className="settings-key-cancel-btn" onClick={() => { setEditing(false); setInputVal(""); }}>
            取消
          </button>
        </div>
      )}
    </div>
  );
}

function maskKey(s: string): string {
  if (!s || s.length <= 16) return "••••••••";
  return s.slice(0, 8) + "..." + s.slice(-4);
}

// ---- 输出 ----

function OutputTab({ config, update }: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(key: K, value: MyriadMindConfig[K]) => void;
}) {
  return (
    <>
      <SettingsSection title="笔记输出">
        <Input
          label="笔记输出目录"
          value={config.output.note_dir}
          placeholder="留空则输出到当前目录。例: D:/Notes/ 或 ./大衍决残卷/"
          hint="绝对路径或相对路径，可按主题自动分子目录。选在云盘文件夹可跨设备同步。"
          onChange={(e) => update("output", { ...config.output, note_dir: e.target.value })}
        />
      </SettingsSection>

      <SettingsSection title="文件管理">
        <Toggle
          label="自动清理临时文件"
          description="处理完成后删除 /tmp 中的视频、音频、字幕、截图"
          checked={config.output.cleanup_temp}
          onChange={(v) => update("output", { ...config.output, cleanup_temp: v })}
        />
      </SettingsSection>

      <SettingsSection title="笔记元信息">
        <Toggle
          label="笔记末尾添加元信息"
          description="记录生成时间、模型、Token 消耗等"
          checked={config.output.note_metadata}
          onChange={(v) => update("output", { ...config.output, note_metadata: v })}
        />
        <Toggle
          label="调试元信息"
          description="额外输出处理链路详情（决策链路、各步骤消耗）"
          checked={config.output.debug_metadata}
          onChange={(v) => update("output", { ...config.output, debug_metadata: v })}
        />
      </SettingsSection>
    </>
  );
}

// ---- 关于 ----

function AboutTab() {
  return (
    <>
      <SettingsSection title="大衍决">
        <div style={{ textAlign: "center", padding: "20px 0" }}>
          <span style={{ fontSize: 48 }}>🧘</span>
          <h2 style={{ fontSize: 20, fontWeight: 700, margin: "8px 0 4px", color: "var(--text)" }}>大衍决</h2>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 4 }}>Myriad Mind v0.1.0</p>
          <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>神识一扫，万物皆可为笔记</p>
        </div>
      </SettingsSection>

      <SettingsSection title="技术栈">
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
          {[
            ["前端", "React 19 + TypeScript"],
            ["后端", "Rust (Tauri 2.x)"],
            ["AI", "Claude API (Anthropic)"],
            ["视频", "Python + FFmpeg + yt-dlp"],
            ["ASR", "faster-whisper"],
            ["存储", "Markdown + SQLite"],
          ].map(([k, v]) => (
            <div key={k} style={{ padding: "8px 12px", background: "var(--bg-root)", borderRadius: 6, border: "1px solid var(--border)" }}>
              <p style={{ fontSize: 10, color: "var(--text-muted)", marginBottom: 2 }}>{k}</p>
              <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>{v}</p>
            </div>
          ))}
        </div>
      </SettingsSection>

      <SettingsSection title="开源协议">
        <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          MIT License — 免费开源，保留上游版权声明。
        </p>
        <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
          本工具仅供个人学习、研究和教育目的。
        </p>
      </SettingsSection>
    </>
  );
}

// ---- Pill 按钮 ----

function PillButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      className={`settings-pill${active ? " settings-pill-active" : ""}`}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
