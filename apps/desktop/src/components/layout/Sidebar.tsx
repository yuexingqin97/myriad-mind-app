import { NavButton } from "@/components/layout/NavButton";
import appIcon from "@/assets/icons/myriad-mind-whale-icon-concept.png";

// ---- Props ----

interface SidebarProps {
  activeView: string;
  onNavigate: (view: "input" | "dashboard" | "settings") => void;
}

// ---- Component ----

export function Sidebar({ activeView, onNavigate }: SidebarProps) {
  return (
    <nav className="sidebar">
      <div className="sidebar-brand" onClick={() => onNavigate("input")}>
        <img src={appIcon} alt="大衍决" className="sidebar-brand-icon" />
        <div>
          <h1>大衍决</h1>
          <p>Myriad Mind</p>
        </div>
      </div>

      <div className="sidebar-nav">
        <NavButton
          active={activeView === "input"}
          icon="📥"
          label="炼化"
          hotkey="1"
          onClick={() => onNavigate("input")}
        />
        {/* 修为面板暂未真实化（仍是前端假数据），导航入口已隐藏。恢复时取消注释：
        <NavButton active={activeView === "dashboard"} icon="📊" label="修为" hotkey="2" onClick={() => onNavigate("dashboard")} />
        */}
        <NavButton
          active={activeView === "settings"}
          icon="⚙️"
          label="设置"
          hotkey="3"
          onClick={() => onNavigate("settings")}
        />
      </div>

      <div className="sidebar-footer">
        <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
          v0.1.0 · Browser
        </span>
      </div>
    </nav>
  );
}
