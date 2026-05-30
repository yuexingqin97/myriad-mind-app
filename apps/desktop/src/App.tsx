import { useState, useCallback, useEffect } from "react";
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
  Dashboard,
  Button,
  Input,
  Card,
} from "@myriad-mind/ui";
import * as api from "./api";
import { DepsPanel } from "./components/DepsPanel";
import "./App.css";

type View = "input" | "dashboard" | "config";

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

function App() {
  const [view, setView] = useState<View>("config");
  const [ready, setReady] = useState(false);
  const [config, setConfig] = useState<MyriadMindConfig>(DEFAULT_CONFIG);
  const [inputUrl, setInputUrl] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [dashboardData] = useState<DashboardData>(mockDashboardData);
  const [processing, setProcessing] = useState(false);

  // 启动时检测首启 + 加载配置
  useEffect(() => {
    (async () => {
      const first = await api.isFirstLaunch();
      if (!first) {
        try {
          const raw = await api.readConfig();
          if (raw && raw !== "{}") {
            setConfig((prev) => ({ ...prev, ...JSON.parse(raw) }));
          }
          setView("input");
        } catch {
          setView("config");
        }
      } else {
        setView("config");
      }
      setReady(true);
    })();
  }, []);

  // 保存配置
  const handleSaveConfig = useCallback((c: MyriadMindConfig) => {
    setConfig(c);
    api.writeConfig(JSON.stringify(c));
    setView("input");
  }, []);

  // 跳过配置
  const handleConfigDone = useCallback(() => setView("input"), []);

  const handleSubmit = useCallback(async () => {
    if (!inputUrl.trim()) return;
    const classify = classifyInput(inputUrl.trim());
    const estimate = estimateCost(classify, config);

    setStatus(
      `${classify.platform} · 预估 ${estimate.estimatedMinutes} 分钟 · ${Math.round((estimate.inputTokens + estimate.outputTokens) / 1000)}k tokens`
    );
    setProcessing(true);
    setProgress(0);

    const steps = [
      { label: "识别输入模式", pct: 10 },
      { label: "检查环境依赖", pct: 25 },
      { label: "获取媒体内容", pct: 45 },
      { label: "AI 分析生成", pct: 70 },
      { label: "组装学习笔记", pct: 90 },
      { label: "更新修为面板", pct: 100 },
    ];

    for (const step of steps) {
      await new Promise((r) => setTimeout(r, 400 + Math.random() * 300));
      setStatus(step.label);
      setProgress(step.pct);
    }

    setProcessing(false);
    setStatus("✅ 炼化完成 — 笔记已生成");
  }, [inputUrl, config]);

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
          <NavButton active={view === "config"} icon="⚙️" label="配置" hotkey="3" onClick={() => setView("config")} />
        </div>

        <div className="sidebar-footer">
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
            v0.1.0 · Windows
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
            <Dashboard data={dashboardData} />
          </div>
        )}

        {view === "config" && (
          <div className="view-container">
            <h2 className="view-title">⚙️ 配置</h2>
            <p className="view-subtitle">
              配置 Python 路径、ASR 后端、视频解析、功能开关、输出路径等参数
            </p>
            <DepsPanel />
            <ConfigWizard
              config={config}
              onSave={handleSaveConfig}
              onCancel={handleConfigDone}
            />
          </div>
        )}
      </main>
    </div>
  );
}

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
