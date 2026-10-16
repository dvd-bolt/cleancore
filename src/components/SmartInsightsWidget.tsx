import React, { useState, useEffect, useCallback } from "react";
import { ActivePage, MetricSnapshot, ProcessItem, SmartInsight } from "../types";
import { fetchSmartInsights } from "../lib/ipc";
import {
  Sparkles,
  Zap,
  Trash2,
  HardDrive,
  Cpu,
  CheckCircle2,
  ArrowRight,
  RefreshCw,
  FolderCode,
} from "lucide-react";

interface SmartInsightsWidgetProps {
  metrics?: MetricSnapshot | null;
  processes?: ProcessItem[];
  onNavigate: (page: ActivePage, tab?: string) => void;
  className?: string;
  initialInsights?: SmartInsight[];
}

export const SmartInsightsWidget: React.FC<SmartInsightsWidgetProps> = ({
  metrics,
  processes,
  onNavigate,
  className = "",
  initialInsights,
}) => {
  const [insights, setInsights] = useState<SmartInsight[]>(initialInsights || []);
  const [loading, setLoading] = useState<boolean>(false);

  const loadInsights = useCallback(async () => {
    setLoading(true);
    try {
      const processNames = processes?.map((p) => p.name);
      const cpuUsage = metrics?.cpu.value?.total_usage_percent;
      const memUsage = metrics?.memory.value?.usage_percent;
      const diskFree = metrics?.disk.value?.free_bytes;
      const diskTotal = metrics?.disk.value?.total_bytes;

      const data = await fetchSmartInsights({
        processes: processNames,
        cpuUsage,
        memoryUsage: memUsage,
        diskFreeBytes: diskFree,
        diskTotalBytes: diskTotal,
      });
      setInsights(data);
    } catch (err: unknown) {
      console.warn("Не удалось получить рекомендации советника:", err);
    } finally {
      setLoading(false);
    }
  }, [processes, metrics]);

  useEffect(() => {
    if (!initialInsights) {
      void loadInsights();
    }
  }, [loadInsights, initialInsights]);

  const getCategoryIcon = (category: string) => {
    switch (category) {
      case "power":
        return <Zap className="w-4 h-4 text-amber-400" />;
      case "recycle_bin":
        return <Trash2 className="w-4 h-4 text-emerald-400" />;
      case "dev_cache":
        return <FolderCode className="w-4 h-4 text-sky-400" />;
      case "disk":
        return <HardDrive className="w-4 h-4 text-rose-400" />;
      case "memory":
        return <Cpu className="w-4 h-4 text-purple-400" />;
      default:
        return <CheckCircle2 className="w-4 h-4 text-emerald-400" />;
    }
  };

  const getLevelBadge = (level: string) => {
    switch (level) {
      case "critical":
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded border bg-red-950/40 border-red-700/50 text-red-300 font-semibold">
            Критично
          </span>
        );
      case "warning":
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded border bg-amber-950/40 border-amber-700/50 text-amber-300 font-semibold">
            Внимание
          </span>
        );
      case "tip":
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded border bg-sky-950/40 border-sky-700/50 text-sky-300 font-semibold">
            Совет
          </span>
        );
      default:
        return (
          <span className="text-[10px] font-mono px-2 py-0.5 rounded border bg-emerald-950/40 border-emerald-700/50 text-emerald-300 font-semibold">
            Оптимально
          </span>
        );
    }
  };

  const hasOnlyOptimal =
    insights.length === 1 && insights[0].id === "all_systems_optimal";

  return (
    <section
      aria-label="Интеллектуальный советник CleanCore"
      className={`bg-surface border border-line rounded-2xl p-5 shadow-sm space-y-4 ${className}`}
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2.5">
          <div className="w-8 h-8 rounded-lg bg-surface-2 border border-line flex items-center justify-center">
            <Sparkles className="w-4 h-4 text-accent" />
          </div>
          <div>
            <h3 className="font-heading text-sm font-bold text-txt">
              Интеллектуальные рекомендации
            </h3>
            <p className="text-xs text-muted">
              Auto-Pilot Watcher: питание, кэши проектов, Корзина и накопители
            </p>
          </div>
        </div>

        <button
          onClick={() => void loadInsights()}
          disabled={loading}
          aria-label="Обновить рекомендации"
          title="Обновить рекомендации"
          className="p-1.5 rounded-lg bg-surface-2 border border-line text-muted hover:text-txt transition-colors disabled:opacity-40"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${loading ? "animate-spin text-accent" : ""}`} />
        </button>
      </div>

      {/* Cards List */}
      <div className="space-y-3">
        {hasOnlyOptimal ? (
          <div className="flex items-center justify-between p-4 rounded-xl bg-surface-2 border border-line/80">
            <div className="flex items-center gap-3">
              <CheckCircle2 className="w-5 h-5 text-emerald-400 shrink-0" />
              <div>
                <p className="text-xs font-semibold text-txt">
                  {insights[0].title}
                </p>
                <p className="text-[11px] text-muted mt-0.5">
                  {insights[0].description}
                </p>
              </div>
            </div>
            <button
              onClick={() => onNavigate(insights[0].targetPage, insights[0].targetTab || undefined)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface border border-line text-xs text-txt hover:bg-surface-2 hover:border-accent transition-colors font-mono shrink-0 ml-4"
            >
              <span>{insights[0].actionLabel}</span>
              <ArrowRight className="w-3.5 h-3.5 text-accent" />
            </button>
          </div>
        ) : (
          insights.map((insight) => (
            <div
              key={insight.id}
              className="p-4 rounded-xl bg-surface-2 border border-line/80 hover:border-line transition-all flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3"
            >
              <div className="flex items-start gap-3">
                <div className="w-8 h-8 rounded-lg bg-surface border border-line flex items-center justify-center shrink-0 mt-0.5">
                  {getCategoryIcon(insight.category)}
                </div>
                <div>
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-bold text-txt">
                      {insight.title}
                    </span>
                    {getLevelBadge(insight.level)}
                    {insight.estimatedSavings && (
                      <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface border border-line text-accent">
                        {insight.estimatedSavings}
                      </span>
                    )}
                  </div>
                  <p className="text-[11px] text-muted mt-1 leading-relaxed">
                    {insight.description}
                  </p>
                </div>
              </div>

              <div className="shrink-0 self-end sm:self-auto">
                <button
                  onClick={() => onNavigate(insight.targetPage, insight.targetTab || undefined)}
                  className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-surface border border-line text-xs font-medium text-txt hover:bg-surface-2 hover:border-accent transition-colors font-mono"
                >
                  <span>{insight.actionLabel}</span>
                  <ArrowRight className="w-3.5 h-3.5 text-accent" />
                </button>
              </div>
            </div>
          ))
        )}
      </div>
    </section>
  );
};
