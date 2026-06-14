import { useConfig } from "@/hooks/useConfig";
import { useTheme } from "@/hooks/useTheme";
import { Sidebar } from "@/components/layout/Sidebar";
import { InputView } from "@/components/input/InputView";
import { DashboardView } from "@/components/dashboard/DashboardView";
import { SettingsView } from "@/components/settings/SettingsView";
import "./App.css";

// ---- App ----

function App() {
  const { view, setView, config, firstLaunch, finishWizard, saveConfig, reloadConfig } = useConfig();
  useTheme(); // 初始化主题（从 localStorage 读取并应用 data-theme）

  return (
    <div className="app-root">
      <Sidebar activeView={view} onNavigate={setView} />
      <main className="main-content">
        {view === "input" && <InputView config={config} />}
        {view === "dashboard" && <DashboardView />}
        {view === "settings" && (
          <SettingsView
            config={config}
            onSave={saveConfig}
            reloadConfig={reloadConfig}
            firstLaunch={firstLaunch}
            onFinishWizard={finishWizard}
            onNavigateToInput={() => setView("input")}
          />
        )}
      </main>
    </div>
  );
}

export default App;
