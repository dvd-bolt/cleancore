import { useState, useEffect } from "react";
import { ActivePage, AppInfo, AppSettings, MetricSnapshot, ProcessItem } from "./types";
import {
  fetchAppInfo,
  fetchAppSettings,
  fetchMetricSnapshot,
  fetchProcessList,
  saveRefreshInterval,
  togglePauseMetrics,
  subscribeToMetrics,
} from "./lib/ipc";
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
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [metrics, setMetrics] = useState<MetricSnapshot | null>(null);
  const [metricsHistory, setMetricsHistory] = useState<MetricSnapshot[]>([]);
  const [processes, setProcesses] = useState<ProcessItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Инициализация данных при старте
  useEffect(() => {
    let mounted = true;

    async function initialize() {
      try {
        const [info, appSet, initMetrics, procSnapshot] = await Promise.all([
          fetchAppInfo(),
          fetchAppSettings(),
          fetchMetricSnapshot(),
          fetchProcessList(),
        ]);

        if (mounted) {
          setAppInfo(info);
          setSettings(appSet);
          setIsPaused(appSet.is_monitoring_paused);
          setMetrics(initMetrics);
          setMetricsHistory([initMetrics]);
          setProcesses(procSnapshot.processes);
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

  // Подписка на фоновые события обновления метрик от бэкенда Tauri
  useEffect(() => {
    let unsubscribe: (() => void) | undefined;

    async function subscribe() {
      unsubscribe = await subscribeToMetrics((snapshot) => {
        setMetrics(snapshot);
        setMetricsHistory((prev) => {
          const next = [...prev, snapshot];
          return next.slice(-30); // хранить последние 30 точек
        });
      });
    }

    subscribe();

    return () => {
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, []);

  // Периодическое фоновое обновление процессов для mini-table (раз в 10 секунд, если не на паузе)
  useEffect(() => {
    if (isPaused) return;

    const interval = setInterval(async () => {
      try {
        const snapshot = await fetchProcessList();
        setProcesses(snapshot.processes);
      } catch (e) {
        console.warn("Фоновое обновление процессов:", e);
      }
    }, 10000);

    return () => clearInterval(interval);
  }, [isPaused]);

  // Переключение паузы
  const handleTogglePause = async () => {
    const nextPaused = !isPaused;
    setIsPaused(nextPaused);
    try {
      await togglePauseMetrics(nextPaused);
      if (settings) {
        setSettings({ ...settings, is_monitoring_paused: nextPaused });
      }
    } catch (e) {
      console.error("Ошибка переключения паузы:", e);
    }
  };

  // Сохранение нового интервала опроса
  const handleUpdateInterval = async (intervalMs: number) => {
    const updated = await saveRefreshInterval(intervalMs);
    setSettings(updated);
  };

  // Ручное обновление списка процессов
  const handleRefreshProcesses = async () => {
    try {
      const snapshot = await fetchProcessList();
      setProcesses(snapshot.processes);
    } catch (e) {
      console.warn("Ошибка обновления процессов:", e);
    }
  };

  const renderContent = () => {
    if (loading) {
      return (
        <div className="h-full flex flex-col items-center justify-center p-8 text-center text-muted">
          <div className="w-8 h-8 border-2 border-line border-t-accent rounded-full animate-spin mb-3" />
          <span className="text-xs font-mono">Инициализация CleanCore v0.0.2...</span>
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
        return (
          <OverviewPage
            metrics={metrics}
            history={metricsHistory}
            processes={processes}
            onNavigate={setActivePage}
          />
        );
      case "processes":
        return (
          <ProcessesPage
            initialProcesses={processes}
            onRefresh={handleRefreshProcesses}
          />
        );
      case "cleanup":
        return <CleanupPage />;
      case "journal":
        return <JournalPage />;
      case "settings":
        return (
          <SettingsPage
            appInfo={appInfo}
            settings={settings}
            onUpdateInterval={handleUpdateInterval}
          />
        );
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
        onTogglePause={handleTogglePause}
      />

      {/* Main Content Area */}
      <main className="flex-1 overflow-y-auto px-8 py-6">
        <div className="max-w-7xl mx-auto">{renderContent()}</div>
      </main>

      {/* Bottom Status Footer */}
      <Footer appInfo={appInfo} />
    </div>
  );
}

export default App;
