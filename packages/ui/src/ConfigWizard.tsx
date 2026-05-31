// ============================================================
// ConfigWizard — 多步骤配置向导
// 基于 @myriad-mind/core 的 Zod Schema 实现配置表单
// API Key 通过 OS 密钥链管理，不明文存储
// ============================================================

import React, { useState } from "react";
import type { MyriadMindConfig } from "@myriad-mind/core";
import { Button } from "./common/Button.js";
import { Card } from "./common/Card.js";
import { Input, Select, Toggle } from "./common/Input.js";

export interface ConfigWizardProps {
  config: MyriadMindConfig;
  onSave: (config: MyriadMindConfig) => void;
  onCancel?: () => void;
  /** 密钥链操作接口 — 由 app 层注入，实现 OS 密钥链读写 */
  keychain?: KeychainApi;
}

/** 密钥链操作接口 — 抽象 Tauri IPC 和浏览器 localStorage */
export interface KeychainApi {
  check(service: string): Promise<boolean>;
  read(service: string): Promise<string>;
  store(service: string, secret: string): Promise<void>;
}

type StepId = "asr" | "api_keys" | "video" | "features" | "keyframes" | "output" | "post";

const STEPS: Array<{ id: StepId; title: string; icon: string }> = [
  { id: "asr", title: "语音识别", icon: "🎙️" },
  { id: "api_keys", title: "API 密钥", icon: "🔑" },
  { id: "video", title: "视频解析", icon: "📹" },
  { id: "features", title: "功能开关", icon: "⚙️" },
  { id: "keyframes", title: "关键帧", icon: "🖼️" },
  { id: "output", title: "输出设置", icon: "📂" },
  { id: "post", title: "收尾设置", icon: "✨" },
];

export function ConfigWizard({
  config: initialConfig,
  onSave,
  onCancel,
  keychain,
}: ConfigWizardProps) {
  const [step, setStep] = useState(0);
  const [config, setConfig] = useState<MyriadMindConfig>({
    ...initialConfig,
  });

  const update = <K extends keyof MyriadMindConfig>(
    key: K,
    value: MyriadMindConfig[K]
  ) => {
    setConfig((c) => ({ ...c, [key]: value }));
  };

  const current = STEPS[step];
  const isFirst = step === 0;
  const isLast = step === STEPS.length - 1;

  return (
    <Card
      title="⚙️ 配置向导"
      subtitle={`步骤 ${step + 1}/${STEPS.length}: ${current.title}`}
      variant="bordered"
      footer={
        <div className="flex justify-between w-full">
          <div className="flex gap-2">
            {!isFirst && (
              <Button variant="secondary" onClick={() => setStep((s) => s - 1)}>
                ← 上一步
              </Button>
            )}
          </div>
          <div className="flex gap-2">
            {onCancel && (
              <Button variant="ghost" onClick={onCancel}>
                取消
              </Button>
            )}
            {isLast ? (
              <Button onClick={() => onSave(config)}>✅ 保存配置</Button>
            ) : (
              <Button onClick={() => setStep((s) => s + 1)}>下一步 →</Button>
            )}
          </div>
        </div>
      }
    >
      {/* 进度条 */}
      <div className="flex gap-1 mb-6">
        {STEPS.map((s, i) => (
          <div
            key={s.id}
            className={[
              "h-1.5 rounded-full flex-1 transition-colors duration-300",
              i <= step
                ? "bg-indigo-500"
                : "bg-gray-200 dark:bg-gray-700",
            ].join(" ")}
            title={s.title}
          />
        ))}
      </div>

      {/* 步骤内容 */}
      <div className="min-h-[300px]">
        {step === 0 && <ASRStep config={config} update={update} />}
        {step === 1 && <ApiKeysStep keychain={keychain} />}
        {step === 2 && <VideoStep config={config} update={update} />}
        {step === 3 && <FeaturesStep config={config} update={update} />}
        {step === 4 && <KeyframesStep config={config} update={update} />}
        {step === 5 && <OutputStep config={config} update={update} />}
        {step === 6 && <PostProcessStep config={config} update={update} />}
      </div>
    </Card>
  );
}

// ============================================================
// API Key 管理步骤
// ============================================================

/** 密钥服务 ID → 显示信息 */
const KEY_DEFS: Array<{
  service: string;
  label: string;
  description: string;
  placeholder: string;
  required: boolean;
}> = [
  {
    service: "claude-api-key",
    label: "Claude API Key",
    description: "Anthropic 控制台 → API Keys → Create Key。格式: sk-ant-api03-...",
    placeholder: "sk-ant-api03-...",
    required: true,
  },
  {
    service: "ai-douyin-api-key",
    label: "AI Douyin API Key",
    description: "抖音/B 站/小红书视频解析。aidouyin.com 注册获取",
    placeholder: "输入 AI Douyin API Key...",
    required: false,
  },
  {
    service: "tikhub-token",
    label: "TikHub Token",
    description: "视频解析备用方案。tikhub.io 注册获取",
    placeholder: "输入 TikHub Token...",
    required: false,
  },
  {
    service: "volcengine-token",
    label: "火山引擎 Token",
    description: "云端 ASR 后端。火山引擎控制台 → 语音技术 → 获取 Token",
    placeholder: "输入火山引擎 Token...",
    required: false,
  },
];

function ApiKeysStep({ keychain }: { keychain?: KeychainApi }) {
  return (
    <div className="space-y-2">
      <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
        所有密钥存储在 OS 密钥链（Windows Credential Manager），绝不明文落盘。
        <br />
        Claude API Key 为必填项，其余按需配置。
      </p>
      {KEY_DEFS.map((def) => (
        <ApiKeyField
          key={def.service}
          service={def.service}
          label={def.label}
          description={def.description}
          placeholder={def.placeholder}
          required={def.required}
          keychain={keychain}
        />
      ))}
    </div>
  );
}

/** 单个 API Key 字段 — 查询/编辑/存储 */
function ApiKeyField({
  service,
  label,
  description,
  placeholder,
  required,
  keychain,
}: {
  service: string;
  label: string;
  description: string;
  placeholder: string;
  required: boolean;
  keychain?: KeychainApi;
}) {
  const [status, setStatus] = useState<"loading" | "set" | "empty">("loading");
  const [masked, setMasked] = useState("");
  const [editing, setEditing] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 检查密钥链中是否已有此 Key
  React.useEffect(() => {
    if (!keychain) {
      setStatus("empty");
      return;
    }
    keychain
      .check(service)
      .then((exists) => {
        if (exists) {
          return keychain.read(service).then((secret) => {
            setMasked(maskSecret(secret));
            setStatus("set");
          });
        }
        setStatus("empty");
      })
      .catch(() => setStatus("empty"));
  }, [service, keychain]);

  const handleSave = async () => {
    if (!inputValue.trim()) return;
    setSaving(true);
    setError(null);
    try {
      await keychain?.store(service, inputValue.trim());
      setMasked(maskSecret(inputValue.trim()));
      setStatus("set");
      setEditing(false);
      setInputValue("");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    // 存储空字符串等效于删除
    try {
      await keychain?.store(service, "");
      setStatus("empty");
      setMasked("");
      setEditing(false);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div
      style={{
        padding: 12,
        borderRadius: 8,
        border: `1px solid ${status === "set" ? "rgba(74,222,128,0.3)" : required ? "rgba(248,113,113,0.3)" : "rgba(250,204,21,0.3)"}`,
        background: "var(--bg-surface, rgba(255,255,255,0.02))",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 4 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ fontWeight: 600, fontSize: 13 }}>{label}</span>
          {required && <span style={{ fontSize: 10, color: "#f87171" }}>必填</span>}
          {status === "set" && <span style={{ fontSize: 11, color: "#4ade80" }}>✅ 已配置</span>}
          {status === "empty" && !required && <span style={{ fontSize: 11, color: "#facc15" }}>可选</span>}
        </div>
        {!editing && (
          <button
            onClick={() => setEditing(true)}
            style={{
              fontSize: 11,
              padding: "2px 8px",
              borderRadius: 4,
              border: "1px solid var(--border, #333)",
              background: "transparent",
              color: "var(--text-secondary, #aaa)",
              cursor: "pointer",
            }}
          >
            {status === "set" ? "编辑" : "配置"}
          </button>
        )}
      </div>
      <p style={{ fontSize: 11, color: "var(--text-muted, #666)", margin: "0 0 4px 0" }}>
        {description}
      </p>
      {!editing && status === "set" && (
        <p style={{ fontSize: 12, color: "var(--text-secondary, #aaa)", fontFamily: "monospace", margin: 0 }}>
          {masked}
        </p>
      )}
      {editing && (
        <div style={{ marginTop: 8, display: "flex", gap: 8, alignItems: "center" }}>
          <input
            type="password"
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            placeholder={placeholder}
            onKeyDown={(e) => e.key === "Enter" && handleSave()}
            style={{
              flex: 1,
              padding: "6px 10px",
              fontSize: 12,
              borderRadius: 6,
              border: "1px solid var(--border, #333)",
              background: "var(--bg-root, #111)",
              color: "var(--text, #eee)",
              outline: "none",
            }}
          />
          <button
            onClick={handleSave}
            disabled={!inputValue.trim() || saving}
            style={{
              padding: "6px 12px",
              fontSize: 11,
              borderRadius: 6,
              border: "none",
              background: "var(--accent, #818cf8)",
              color: "white",
              cursor: inputValue.trim() ? "pointer" : "not-allowed",
              opacity: inputValue.trim() ? 1 : 0.5,
            }}
          >
            {saving ? "保存中…" : "保存"}
          </button>
          {status === "set" && (
            <button
              onClick={handleDelete}
              style={{
                padding: "6px 12px",
                fontSize: 11,
                borderRadius: 6,
                border: "1px solid rgba(248,113,113,0.3)",
                background: "transparent",
                color: "#f87171",
                cursor: "pointer",
              }}
            >
              删除
            </button>
          )}
          <button
            onClick={() => { setEditing(false); setInputValue(""); setError(null); }}
            style={{
              padding: "6px 12px",
              fontSize: 11,
              borderRadius: 6,
              border: "1px solid var(--border, #333)",
              background: "transparent",
              color: "var(--text-secondary, #aaa)",
              cursor: "pointer",
            }}
          >
            取消
          </button>
        </div>
      )}
      {error && (
        <p style={{ fontSize: 11, color: "#f87171", marginTop: 4 }}>{error}</p>
      )}
    </div>
  );
}

/** 脱敏显示密钥: 前 8 字符 + ... + 后 4 字符 */
function maskSecret(secret: string): string {
  if (!secret) return "";
  if (secret.length <= 16) return "••••••••";
  return secret.slice(0, 8) + "..." + secret.slice(-4);
}

// ============================================================
// 其他步骤组件
// ============================================================

function ASRStep({
  config,
  update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(
    key: K,
    value: MyriadMindConfig[K]
  ) => void;
}) {
  return (
    <div className="space-y-4">
      <Input
        label="Python 路径（留空自动探测）"
        value={config.python_path ?? ""}
        placeholder="自动探测系统 Python… 例：C:/Users/xxx/.cache/myriad-mind/faster-whisper-venv/Scripts/python.exe"
        hint="对应原 FW_PYTHON 环境变量。留空则搜索 PATH 中的 python3/python"
        onChange={(e) => update("python_path", e.target.value)}
      />
      <Select
        label="ASR 后端"
        options={[
          { value: "faster-whisper", label: "faster-whisper（本地运行，免费）" },
          { value: "volcengine", label: "火山引擎 VC API（云端，需付费）" },
        ]}
        value={config.asr.backend}
        onChange={(e) =>
          update("asr", {
            ...config.asr,
            backend: e.target.value as "faster-whisper" | "volcengine",
          })
        }
      />

      {config.asr.backend === "faster-whisper" && (
        <>
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
            onChange={(e) =>
              update("asr", {
                ...config.asr,
                faster_whisper: {
                  model_size: e.target.value as
                    | "tiny"
                    | "base"
                    | "small"
                    | "medium"
                    | "large-v3",
                  device: config.asr.faster_whisper?.device ?? "auto",
                  compute_type: config.asr.faster_whisper?.compute_type,
                },
              })
            }
          />
          <Select
            label="运行设备"
            options={[
              { value: "auto", label: "auto — 自动检测" },
              { value: "cpu", label: "CPU" },
              { value: "cuda", label: "CUDA（NVIDIA GPU）" },
            ]}
            value={config.asr.faster_whisper?.device ?? "auto"}
            onChange={(e) =>
              update("asr", {
                ...config.asr,
                faster_whisper: {
                  model_size:
                    config.asr.faster_whisper?.model_size ?? "medium",
                  device: e.target.value as "auto" | "cpu" | "cuda",
                  compute_type: config.asr.faster_whisper?.compute_type,
                },
              })
            }
          />
        </>
      )}

      {config.asr.backend === "volcengine" && (
        <>
          <Input
            label="火山引擎 App ID"
            value={config.asr.volcengine?.appid ?? ""}
            placeholder="火山引擎控制台→语音技术→应用ID"
            hint="Token 请在「API 密钥」步骤中配置，存储在 OS 密钥链"
            onChange={(e) =>
              update("asr", {
                ...config.asr,
                volcengine: {
                  token_keyring_id: "volcengine-token",
                  appid: e.target.value,
                },
              })
            }
          />
        </>
      )}
    </div>
  );
}

function VideoStep({
  config,
  update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(
    key: K,
    value: MyriadMindConfig[K]
  ) => void;
}) {
  return (
    <div className="space-y-4">
      <Select
        label="视频信息提供商"
        options={[
          {
            value: "ai-douyin",
            label: "AI Douyin（代理，注册即有免费额度，推荐）",
          },
          { value: "tikhub", label: "TikHub（自建 Token，更灵活）" },
        ]}
        value={config.video.provider}
        onChange={(e) =>
          update("video", {
            provider: e.target.value as "ai-douyin" | "tikhub",
          })
        }
      />
      <p className="text-xs text-gray-400">
        抖音 / 小红书 / B 站视频解析需要 API Key（已在「API 密钥」步骤配置）。
        YouTube 不需要额外 Key。
      </p>
    </div>
  );
}

function FeaturesStep({
  config,
  update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(
    key: K,
    value: MyriadMindConfig[K]
  ) => void;
}) {
  const toggleFeature = (key: keyof typeof config.features) => {
    update("features", {
      ...config.features,
      [key]: !config.features[key],
    });
  };

  return (
    <div className="space-y-2">
      <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
        控制笔记生成时包含哪些内容（全部默认开启）
      </p>
      <Toggle
        label="关键帧截图"
        description="从视频截图并嵌入笔记，帮助理解画面内容"
        checked={config.features.keyframes}
        onChange={() => toggleFeature("keyframes")}
      />
      <Toggle
        label="Mermaid 图表"
        description="自动绘制架构图、流程图、时序图"
        checked={config.features.mermaid}
        onChange={() => toggleFeature("mermaid")}
      />
      <Toggle
        label="扩展学习资源"
        description="推荐相关文档、视频、GitHub 仓库等"
        checked={config.features.resources}
        onChange={() => toggleFeature("resources")}
      />
      <Toggle
        label="评论区精华"
        description="自动提取视频评论区高质量讨论"
        checked={config.features.comments}
        onChange={() => toggleFeature("comments")}
      />
      <Toggle
        label="阅读时长与难度评级"
        description="在笔记开头标注推荐阅读时间和内容难度"
        checked={config.features.reading_info}
        onChange={() => toggleFeature("reading_info")}
      />
      <Toggle
        label="灵力预估"
        description="处理前显示 Token/时间/费用预估"
        checked={config.features.estimation}
        onChange={() => toggleFeature("estimation")}
      />
    </div>
  );
}

function KeyframesStep({
  config,
  update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(
    key: K,
    value: MyriadMindConfig[K]
  ) => void;
}) {
  return (
    <div className="space-y-4">
      <Input
        label="截图间隔（秒）"
        type="number"
        min={5}
        max={300}
        value={config.keyframes.interval}
        onChange={(e) =>
          update("keyframes", {
            ...config.keyframes,
            interval: Math.max(5, Math.min(300, Number(e.target.value) || 30)),
          })
        }
        hint="每隔 N 秒截一帧，范围 5-300"
      />
      <Input
        label="最大截图数"
        type="number"
        min={1}
        max={200}
        value={config.keyframes.max_frames}
        onChange={(e) =>
          update("keyframes", {
            ...config.keyframes,
            max_frames: Math.max(
              1,
              Math.min(200, Number(e.target.value) || 50)
            ),
          })
        }
        hint="超过此数不再截取，范围 1-200"
      />
      <Select
        label="截图模式"
        options={[
          { value: "interval", label: "固定间隔 — 每 N 秒截一张" },
          { value: "scene", label: "场景检测 — 画面变化时截取" },
          { value: "both", label: "两者结合 — 推荐" },
        ]}
        value={config.keyframes.mode}
        onChange={(e) =>
          update("keyframes", {
            ...config.keyframes,
            mode: e.target.value as "interval" | "scene" | "both",
          })
        }
      />
    </div>
  );
}

function OutputStep({
  config,
  update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(
    key: K,
    value: MyriadMindConfig[K]
  ) => void;
}) {
  return (
    <div className="space-y-4">
      <Input
        label="笔记输出目录"
        value={config.output.note_dir}
        placeholder="留空则输出到当前目录；例：D:/Notes/ 或 ./大衍决残卷/"
        onChange={(e) =>
          update("output", { ...config.output, note_dir: e.target.value })
        }
        hint="绝对路径或相对路径，可按主题自动分子目录"
      />
      <Toggle
        label="自动清理临时文件"
        description="处理完成后删除 /tmp 中的视频、音频、字幕、截图"
        checked={config.output.cleanup_temp}
        onChange={(checked) =>
          update("output", { ...config.output, cleanup_temp: checked })
        }
      />
      <Toggle
        label="笔记末尾添加元信息"
        description="记录生成时间、模型、Token 消耗、更新日志等"
        checked={config.output.note_metadata}
        onChange={(checked) =>
          update("output", { ...config.output, note_metadata: checked })
        }
      />
      <Toggle
        label="调试元信息"
        description="额外输出处理链路详情（决策链路、各步骤消耗）"
        checked={config.output.debug_metadata}
        onChange={(checked) =>
          update("output", { ...config.output, debug_metadata: checked })
        }
      />
    </div>
  );
}

function PostProcessStep({
  config,
  update,
}: {
  config: MyriadMindConfig;
  update: <K extends keyof MyriadMindConfig>(
    key: K,
    value: MyriadMindConfig[K]
  ) => void;
}) {
  return (
    <div className="space-y-2">
      <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
        笔记生成完成后的收尾操作
      </p>
      <Toggle
        label="自动更新修为面板"
        description="每次生成笔记后自动刷新统计数据和成就进度"
        checked={config.post_process.auto_update_panel}
        onChange={(checked) =>
          update("post_process", {
            ...config.post_process,
            auto_update_panel: checked,
          })
        }
      />
      <Toggle
        label="学习路线推荐"
        description="基于当前知识结构推荐下一步学习方向"
        checked={config.post_process.auto_suggest_next}
        onChange={(checked) =>
          update("post_process", {
            ...config.post_process,
            auto_suggest_next: checked,
          })
        }
      />
    </div>
  );
}
