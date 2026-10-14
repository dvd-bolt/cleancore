import { useState, useEffect } from "react";
import { ActivePage, AppInfo, MetricSnapshot } from "./types";
import { fetchAppInfo, fetchInitialMetrics } from "./lib/ipc";
import { Header } from "./components/Header";
import { Footer } from "./components/Footer";
import { OverviewPage } from "./pages/OverviewPage";
import { ProcessesPage } from "./pages/ProcessesPage";
import { CleanupPage } from "./pages/CleanupPage";
import { JournalPage } from "./pages/JournalPage";
import { SettingsPage } from "./pages/SettingsPage";
import { AlertCircle } from "lucide-react";

export function App() {
  const [activePage, setActivePage] = useState<ActivePage>("overview");
  const [isPaused, setIsPaused] = useState(false);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [metrics, setMetrics] = useState<MetricSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;

    async function initialize() {
      try {
        const [info, initMetrics] = await Promise.all([
          fetchAppInfo(),
          fetchInitialMetrics(),
        ]);
        if (mounted) {
          setAppInfo(info);
          setMetrics(initMetrics);
          setLoading(false);
        }
      } catch (err) {
        if (mounted) {
          setError(err instanceof Error ? err.message : String(err));
          setLoading(false);
        }
      }
    }

    initialize();

    return () => {
      mounted = false;
    };
  }, []);

  const renderContent = () => {
    if (loading) {
      return (
        <div className="h-full flex flex-col items-center justify-center p-8 text-center text-muted">
          <div className="w-8 h-8 border-2 border-line border-t-accent rounded-full animate-spin mb-3" />
          <span className="text-xs font-mono">Инициализация CleanCore...</span>
        </div>
      );
    }

    if (error) {
      return (
        <div className="h-full flex flex-col items-center justify-center p-8 text-center">
          <div className="w-12 h-12 rounded-full bg-red-950/40 border border-red-700/50 flex items-center justify-center text-red-400 mb-3">
            <AlertCircle className="w-6 h-6" />
          </div>
          <h3 className="font-heading text-base font-semibold text-txt">
            Ошибка инициализации
          </h3>
          <p className="text-xs text-muted max-w-md mt-1 mb-4 font-mono">
            {error}
          </p>
          <button
            onClick={() => window.location.reload()}
            className="px-4 py-2 rounded-lg bg-surface border border-line text-xs text-txt hover:bg-surface-2 transition-all font-mono"
          >
            Повторить попытку
          </button>
        </div>
      );
    }

    switch (activePage) {
      case "overview":
        return <OverviewPage metrics={metrics} onNavigate={setActivePage} />;
      case "processes":
        return <ProcessesPage />;
      case "cleanup":
        return <CleanupPage />;
      case "journal":
        return <JournalPage />;
      case "settings":
        return <SettingsPage appInfo={appInfo} />;
      default:
        return null;
    }
  };

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-bg text-txt select-none">
      {/* Top Header */}
      <Header
        activePage={activePage}
        onPageChange={setActivePage}
        isPaused={isPaused}
        onTogglePause={() => setIsPaused(!isPaused)}
      />

      {/* Main Content Area */}
      <main className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-7xl mx-auto">
          {renderContent()}
        </div>
      </main>

      {/* Bottom Status Footer */}
      <Footer appInfo={appInfo} />
    </div>
  );
}

export default App;
