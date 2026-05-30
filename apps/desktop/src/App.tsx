import { useState } from "react";
import { UI_VERSION } from "@myriad-mind/ui";
import { DEFAULT_CONFIG, cultivationEmoji, calculateCultivation, computeStats } from "@myriad-mind/core";

function App() {
  const [message] = useState("大衍决 · 神识一扫，万物皆可为笔记");

  return (
    <div className="app">
      <header className="app-header">
        <h1>🧘 大衍决</h1>
        <p className="tagline">{message}</p>
        <div className="version-info">
          <span>UI v{UI_VERSION}</span>
        </div>
      </header>
      <main className="app-main">
        <section className="status-card">
          <h2>修为面板</h2>
          <p>
            {cultivationEmoji("炼气期")} 炼气期 — 万事开头难，先炼化第一篇笔记吧！
          </p>
          <p className="hint">
            配置输出目录 <code>{DEFAULT_CONFIG.output.note_dir || "(未设置)"}</code> 以开始修炼
          </p>
        </section>
      </main>
    </div>
  );
}

export default App;
