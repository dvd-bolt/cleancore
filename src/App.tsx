import { useState, useEffect, useRef } from "react";
import { ActivePage, AppInfo, AppSettings, MetricSnapshot, ProcessItem } from "./types";
import {
  fetchAppInfo,
  fetchAppSettings,
  fetchMetricSnapshot,
  fetchProcessList,
  saveRefreshInterval,
  togglePauseMetrics,
  subscribeToMetrics,
  isTauriEnvironment,
} from "./lib/ipc";
import { Header } from "./components/Header";
import { Footer } from "./components/Footer";
import { OverviewPage } from "./pages/OverviewPage";
import { ProcessesPage } from "./pages/ProcessesPage";
import { DiskPage } from "./pages/DiskPage";
import { CleanupPage } from "./pages/CleanupPage";
import { JournalPage } from "./pages/JournalPage";
import { SettingsPage } from "./pages/SettingsPage";
import { AlertCircle } from "lucide-react";

export function App() {
  const [activePage, setActivePage] = useState<ActivePage>("overview");
  const [isPaused, setIsPaused] = useState(false);
  const pausePending = useRef(false);
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
    let disposed = false;

    async function subscribe() {
      const stop = await subscribeToMetrics((snapshot) => {
        if (disposed) return;
        setMetrics(snapshot);
        setMetricsHistory((prev) => {
          const next = [...prev, snapshot];
          const newest = Date.parse(snapshot.timestamp_utc);
          return next.filter((item) => Date.parse(item.timestamp_utc) >= newest - 60000).slice(-121);
        });
      });
      if (disposed) stop();
      else unsubscribe = stop;
    }

    subscribe().catch((err) => { if (!disposed) setError(err instanceof Error ? err.message : String(err)); });

    return () => {
      disposed = true;
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, []);

  // Периодическое фоновое обновление процессов для mini-table (раз в 10 секунд, если не на паузе)
  useEffect(() => {
    if (isPaused) return;

    let disposed = false;
    let pending = false;
    const interval = setInterval(async () => {
      if (pending) return;
      pending = true;
      try {
        const snapshot = await fetchProcessList();
        if (!disposed) setProcesses(snapshot.processes);
      } catch (e) {
        if (!disposed) setError(e instanceof Error ? e.message : String(e));
      } finally { pending = false; }
    }, 10000);

    return () => { disposed = true; clearInterval(interval); };
  }, [isPaused]);

  // Переключение паузы
  const handleTogglePause = async () => {
    if (pausePending.current) return;
    pausePending.current = true;
    const nextPaused = !isPaused;
    try {
      await togglePauseMetrics(nextPaused);
      setIsPaused(nextPaused);
      if (settings) {
        setSettings({ ...settings, is_monitoring_paused: nextPaused });
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { pausePending.current = false; }
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
      return snapshot;
    } catch (e) {
      throw e instanceof Error ? e : new Error(String(e));
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
      case "disk":
        return <DiskPage />;
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
        <div className="max-w-7xl mx-auto">
          {!isTauriEnvironment() && <p role="status" className="text-xs text-muted mb-4">Предпросмотр интерфейса в браузере: системный backend и SQLite не подключены.</p>}
          {appInfo && isTauriEnvironment() && !appInfo.settings_persistent && <p role="alert" className="text-sm text-warn mb-4">Постоянная SQLite недоступна: настройки действуют только в памяти, исполнение очистки заблокировано.</p>}
          {renderContent()}
        </div>
      </main>

      {/* Bottom Status Footer */}
      <Footer appInfo={appInfo} />
    </div>
  );
}

export default App;
