import React, { useState, useCallback, useEffect, useRef } from "react";
import {
  type MyriadMindConfig,
  type DashboardData,
  DEFAULT_CONFIG,
  classifyInput,
  estimateCost,
  calculateCultivation,
  checkAchievements,
} from "@myriad-mind/core";
import {
  ConfigWizard,
  SettingsPage,
  Dashboard,
  Button,
  Input,
  Card,
  type KeychainApi,
} from "@myriad-mind/ui";
import * as api from "./api";
import { DepsPanel } from "./components/DepsPanel";
import "./App.css";

type View = "input" | "dashboard" | "settings";

// ---- Mock 数据（浏览器预览用）----

const mockDashboardData: DashboardData = {
  cultivation: calculateCultivation({
    totalNotes: 3, beginnerNotes: 1, intermediateNotes: 1, advancedNotes: 1,
    uniqueSources: 2, techStacks: 3, totalHours: 5.2, avgReadingTime: 15,
    topTags: [{ tag: "#Rust", count: 2 }, { tag: "#Bevy", count: 2 }, { tag: "#ECS", count: 1 }],
  }),
  stats: {
    totalNotes: 3, beginnerNotes: 1, intermediateNotes: 1, advancedNotes: 1,
    uniqueSources: 2, techStacks: 3, totalHours: 5.2, avgReadingTime: 15,
    topTags: [{ tag: "#Rust", count: 2 }, { tag: "#Bevy", count: 2 }, { tag: "#ECS", count: 1 }],
  },
  achievements: checkAchievements(
    { totalNotes: 3, beginnerNotes: 1, intermediateNotes: 1, advancedNotes: 1,
      uniqueSources: 2, techStacks: 3, totalHours: 5.2, avgReadingTime: 15,
      topTags: [{ tag: "#Rust", count: 2 }] }, []
  ),
  recentNotes: [
    { title: "Bevy ECS 架构源码分析", date: "2026-05-20", type: "video" },
    { title: "Rust 异步编程入门", date: "2026-05-18", type: "article" },
    { title: "UE5 蓝图入门教程", date: "2026-05-15", type: "video" },
  ],
  streak: 3,
};

const mockPipelineSteps = [
  { label: "识别输入模式", pct: 10 },
  { label: "检查环境依赖", pct: 25 },
  { label: "获取媒体内容", pct: 45 },
  { label: "AI 分析生成", pct: 70 },
  { label: "组装学习笔记", pct: 90 },
  { label: "更新修为面板", pct: 100 },
];

// ---- Helpers ----

let _isTauri: boolean | null = null;
async function isTauri(): Promise<boolean> {
  if (_isTauri !== null) return _isTauri;
  try {
    await import("@tauri-apps/api/core");
    _isTauri = true;
  } catch {
    _isTauri = false;
  }
  return _isTauri;
}

// ---- App ----

function App() {
  const [view, setView] = useState<View>("input");
  const [firstLaunch, setFirstLaunch] = useState(false);
  const [config, setConfig] = useState<MyriadMindConfig>(DEFAULT_CONFIG);
  const [inputUrl, setInputUrl] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [progressDetail, setProgressDetail] = useState<string | null>(null);
  const [processing, setProcessing] = useState(false);
  const pipelineCancelRef = useRef<(() => void) | null>(null);

  // Keychain 适配器
  const keychainAdapter: KeychainApi = React.useMemo(() => ({
    async check(service: string) {
      const result = await api.checkKeychainEntry(service, "myriad-mind");
      return result.exists;
    },
    async read(service: string) {
      return api.readKeychainEntry(service, "myriad-mind");
    },
    async store(service: string, secret: string) {
      await api.storeKeychainEntry(service, "myriad-mind", secret);
    },
  }), []);

  // 启动时
  useEffect(() => {
    (async () => {
      if (await isTauri()) {
        const first = await api.isFirstLaunch();
        if (first) {
          setFirstLaunch(true);
          setView("settings"); // 首启跳转设置页
        } else {
          try {
            const raw = await api.readConfig();
            if (raw && raw !== "{}") {
              setConfig((prev) => ({ ...prev, ...JSON.parse(raw) }));
            }
          } catch { /* ignore */ }
        }
      }
    })();
  }, []);

  // 保存设置
  const handleSaveConfig = useCallback((c: MyriadMindConfig) => {
    setConfig(c);
    api.writeConfig(JSON.stringify(c));
    if (firstLaunch) {
      setFirstLaunch(false);
      setView("input");
    }
  }, [firstLaunch]);

  // ---- 炼化提交 ----

  const handleSubmit = useCallback(async () => {
    if (!inputUrl.trim() || processing) return;
    const classify = classifyInput(inputUrl.trim());
    const estimate = estimateCost(classify, config);

    setStatus(
      `${classify.platform} · 预估 ${estimate.estimatedMinutes} 分钟 · ${Math.round((estimate.inputTokens + estimate.outputTokens) / 1000)}k tokens`
    );
    setProgressDetail(null);
    setProcessing(true);
    setProgress(0);

    if (await isTauri()) {
      const unlisten = api.listenPipelineProgress((event) => {
        setProgress(Math.round(event.percent));
        setStatus(event.label);
        if (event.detail) setProgressDetail(event.detail);
        if (event.status === "completed" && event.step === "completed") {
          setProcessing(false);
          unlisten();
          pipelineCancelRef.current = null;
        }
        if (event.status === "failed") {
          setProcessing(false);
          setProgressDetail(`❌ ${event.label}`);
          unlisten();
          pipelineCancelRef.current = null;
        }
      });
      pipelineCancelRef.current = unlisten;

      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke("execute_pipeline", {
          input: inputUrl.trim(),
          mode: classify.mode,
          pythonPath: config.python_path || null,
          noteDir: config.output.note_dir || "",
        }) as { success: boolean; mode: string; duration_seconds: number };
        if (result.success) {
          setProgress(100);
          setStatus(`✅ 炼化完成 — ${result.mode} · ${result.duration_seconds.toFixed(1)}s`);
        }
      } catch (e) {
        setStatus(`❌ 管线执行失败: ${e}`);
        setProcessing(false);
        unlisten();
      }
    } else {
      for (const step of mockPipelineSteps) {
        await new Promise((r) => setTimeout(r, 350 + Math.random() * 250));
        setStatus(step.label);
        setProgress(step.pct);
      }
      setProcessing(false);
      setStatus("✅ 炼化完成 — 笔记已生成（浏览器模拟）");
      setProgressDetail(`识别为 ${classify.platform}，模拟耗时 ${(mockPipelineSteps.length * 0.5).toFixed(1)}s`);
    }
  }, [inputUrl, config, processing]);

  useEffect(() => {
    return () => { pipelineCancelRef.current?.(); };
  }, []);

  // ---- Render ----

  // 首次启动 → 显示向导
  const showWizard = firstLaunch && view === "settings";

  return (
    <div className="app-root">
      {/* 侧边栏 */}
      <nav className="sidebar">
        <div className="sidebar-brand" onClick={() => setView("input")}>
          <span style={{ fontSize: 24 }}>🧘</span>
          <div>
            <h1>大衍决</h1>
            <p>Myriad Mind</p>
          </div>
        </div>

        <div className="sidebar-nav">
          <NavButton active={view === "input"} icon="📥" label="炼化" hotkey="1" onClick={() => setView("input")} />
          <NavButton active={view === "dashboard"} icon="📊" label="修为" hotkey="2" onClick={() => setView("dashboard")} />
          <NavButton active={view === "settings"} icon="⚙️" label="设置" hotkey="3" onClick={() => setView("settings")} />
        </div>

        <div className="sidebar-footer">
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
            v0.1.0 · Browser
          </span>
        </div>
      </nav>

      {/* 主内容 */}
      <main className="main-content">
        {view === "input" && (
          <div className="view-container">
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
                  onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
                />
                <Button onClick={handleSubmit} disabled={!inputUrl.trim() || processing} loading={processing}>
                  炼化
                </Button>
              </div>

              {status && (
                <div className="status-bar">
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <span>{status}</span>
                    {processing && <span style={{ color: "#818cf8", fontSize: 12 }}>{progress}%</span>}
                  </div>
                  {progressDetail && (
                    <p style={{ fontSize: 11, color: "var(--text-muted)", margin: "4px 0 0" }}>{progressDetail}</p>
                  )}
                  {processing && (
                    <div className="progress-bar">
                      <div className="progress-fill" style={{ width: `${progress}%` }} />
                    </div>
                  )}
                </div>
              )}
            </div>

            <div style={{ marginBottom: 16 }}>
              <h3 style={{ fontSize: 13, fontWeight: 600, color: "#a0a0c0", marginBottom: 10, textTransform: "uppercase", letterSpacing: "0.05em" }}>
                🎯 支持的输入类型
              </h3>
              <div className="quick-hints">
                <HintCard icon="🎬" title="在线视频" desc="B 站 / YouTube / 抖音 / 小红书" />
                <HintCard icon="📄" title="在线文章" desc="知乎 / CSDN / 掘金 / Wiki" />
                <HintCard icon="📂" title="本地文件" desc="视频 / 音频 / Markdown / 纯文本" />
                <HintCard icon="💻" title="代码项目" desc="GitHub 仓库 / 本地代码目录" />
              </div>
            </div>

            <Card title="⚡ 处理管线" subtitle="桌面端完整数据流" variant="elevated">
              <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
                {["模式识别", "灵力预估", "视频下载", "音频提取", "ASR 转写", "关键帧", "AI 笔记", "清理", "面板更新"].map((step, i) => (
                  <span key={i} style={{ padding: "4px 10px", fontSize: 11, borderRadius: 6, background: "var(--bg-root)", border: "1px solid var(--border)", color: "var(--text-secondary)" }}>
                    {i + 1}. {step}
                  </span>
                ))}
              </div>
            </Card>
          </div>
        )}

        {view === "dashboard" && (
          <div className="view-container">
            <h2 className="view-title">📊 修为面板</h2>
            <p className="view-subtitle">修炼进度 · 统计面板 · 成就系统</p>
            <Dashboard data={mockDashboardData} />
          </div>
        )}

        {view === "settings" && (
          <div className="view-container">
            <h2 className="view-title">⚙️ 设置</h2>
            <p className="view-subtitle">管理应用配置、API 密钥和功能偏好</p>

            {showWizard ? (
              <ConfigWizard
                config={config}
                onSave={handleSaveConfig}
                onCancel={() => { setFirstLaunch(false); }}
                keychain={keychainAdapter}
              />
            ) : (
              <>
                <DepsPanel pythonPath={config.python_path || undefined} />
                <SettingsPage
                  config={config}
                  onSave={handleSaveConfig}
                  keychain={keychainAdapter}
                />
              </>
            )}
          </div>
        )}
      </main>
    </div>
  );
}

// ---- Sub-components ----

function NavButton({ active, icon, label, hotkey, onClick }: {
  active: boolean; icon: string; label: string; hotkey: string; onClick: () => void;
}) {
  return (
    <button className={`nav-btn${active ? " nav-btn-active" : ""}`} onClick={onClick}>
      <span style={{ fontSize: 16 }}>{icon}</span>
      <span style={{ flex: 1 }}>{label}</span>
      <span style={{ fontSize: 10, opacity: 0.4 }}>{hotkey}</span>
    </button>
  );
}

function HintCard({ icon, title, desc }: { icon: string; title: string; desc: string }) {
  return (
    <div className="hint-card">
      <span style={{ fontSize: 22 }}>{icon}</span>
      <div>
        <p style={{ fontWeight: 600, fontSize: 13, color: "var(--text)" }}>{title}</p>
        <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{desc}</p>
      </div>
    </div>
  );
}

export default App;
