import React from "react";
import { MetricSnapshot, ActivePage } from "../types";
import { MetricCard } from "../components/MetricCard";
import { ActivityChart } from "../components/ActivityChart";
import { CleanupCard } from "../components/CleanupCard";
import { ProcessMiniTable } from "../components/ProcessMiniTable";
import { Cpu, HardDrive, MemoryStick, Network, Wifi } from "lucide-react";

interface OverviewPageProps {
  metrics: MetricSnapshot | null;
  onNavigate: (page: ActivePage) => void;
}

export const OverviewPage: React.FC<OverviewPageProps> = ({ metrics, onNavigate }) => {
  return (
    <div className="space-y-6">
      {/* Page Heading */}
      <div className="flex flex-wrap items-end justify-between gap-4 pb-2 border-b border-line/40">
        <div>
          <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
            Обзор системы
          </span>
          <h2 className="font-heading text-2xl font-bold text-txt">
            Всё важное. <span className="text-dim">На одном экране.</span>
          </h2>
          <p className="text-xs text-muted mt-1">
            Нагрузка, процессы и свободное место — без скрытых действий и лишнего шума.
          </p>
        </div>

        <div className="text-right font-mono text-[11px] text-dim">
          <span>Последнее обновление: </span>
          <strong className="text-txt">
            {metrics?.cpu.status === "Available" ? metrics.timestamp_utc : "Ожидание данных"}
          </strong>
        </div>
      </div>

      {/* Metric Cards Row */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <MetricCard
          title="Процессор"
          tag="CPU"
          icon={<Cpu className="w-4 h-4" />}
          field={metrics?.cpu || { status: "NotMeasured" }}
          formattedValue={
            metrics?.cpu.value?.total_usage_percent !== undefined
              ? metrics.cpu.value.total_usage_percent.toFixed(1)
              : "—"
          }
          unit="%"
          subText={metrics?.cpu.value ? `${metrics.cpu.value.core_count} ядер` : undefined}
          progressPercent={metrics?.cpu.value?.total_usage_percent}
        />

        <MetricCard
          title="Память"
          tag="RAM"
          icon={<MemoryStick className="w-4 h-4" />}
          field={metrics?.memory || { status: "NotMeasured" }}
          formattedValue={
            metrics?.memory.value?.used_bytes !== undefined
              ? (metrics.memory.value.used_bytes / (1024 * 1024 * 1024)).toFixed(1)
              : "—"
          }
          unit="ГБ"
          subText={
            metrics?.memory.value
              ? `из ${(metrics.memory.value.total_bytes / (1024 * 1024 * 1024)).toFixed(0)} ГБ`
              : undefined
          }
          progressPercent={metrics?.memory.value?.usage_percent}
        />

        <MetricCard
          title="Свободно на диске"
          tag="DISK"
          icon={<HardDrive className="w-4 h-4" />}
          field={metrics?.disk || { status: "NotMeasured" }}
          formattedValue={
            metrics?.disk.value?.free_bytes !== undefined
              ? (metrics.disk.value.free_bytes / (1024 * 1024 * 1024)).toFixed(0)
              : "—"
          }
          unit="ГБ"
          subText={metrics?.disk.value ? `Диск ${metrics.disk.value.mount_point}` : undefined}
          progressPercent={metrics?.disk.value?.usage_percent}
        />

        <MetricCard
          title="Сетевая активность"
          tag="NET"
          icon={<Network className="w-4 h-4" />}
          field={metrics?.network || { status: "NotMeasured" }}
          formattedValue={
            metrics?.network.value?.rx_bytes_per_sec !== undefined
              ? (metrics.network.value.rx_bytes_per_sec / 1024).toFixed(0)
              : "—"
          }
          unit="КБ/с"
          subText={metrics?.network.value?.interface_name || undefined}
        />
      </div>

      {/* Main Grid: Chart + Cleanup Action */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2">
          <ActivityChart />
        </div>
        <div>
          <CleanupCard onOpenCleanup={() => onNavigate("cleanup")} />
        </div>
      </div>

      {/* Bottom Grid: Process Preview + Network details */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2">
          <ProcessMiniTable onOpenProcesses={() => onNavigate("processes")} />
        </div>

        <article className="bg-surface border border-line rounded-xl p-5 flex flex-col justify-between">
          <div className="flex items-center justify-between mb-3">
            <div>
              <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
                Адаптеры
              </span>
              <h3 className="font-heading text-base font-semibold text-txt">
                Сеть
              </h3>
            </div>
            <Wifi className="w-4 h-4 text-accent" />
          </div>

          <div className="space-y-3 my-2 font-mono text-xs">
            <div className="flex items-center justify-between p-2.5 rounded-lg bg-shell border border-line">
              <span className="text-muted">↓ Приём</span>
              <strong className="text-txt">— КБ/с</strong>
            </div>
            <div className="flex items-center justify-between p-2.5 rounded-lg bg-shell border border-line">
              <span className="text-muted">↑ Передача</span>
              <strong className="text-txt">— КБ/с</strong>
            </div>
          </div>

          <p className="text-[11px] text-dim text-center mt-2">
            Счётчики сетевых интерфейсов подключаются на этапе 2
          </p>
        </article>
      </div>
    </div>
  );
};
