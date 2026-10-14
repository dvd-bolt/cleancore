import React from "react";
import { MetricSnapshot, ProcessItem, ActivePage } from "../types";
import { MetricCard } from "../components/MetricCard";
import { ActivityChart } from "../components/ActivityChart";
import { CleanupCard } from "../components/CleanupCard";
import { ProcessMiniTable } from "../components/ProcessMiniTable";
import { Cpu, HardDrive, MemoryStick, Network, Wifi } from "lucide-react";

interface OverviewPageProps {
  metrics: MetricSnapshot | null;
  history: MetricSnapshot[];
  processes: ProcessItem[];
  onNavigate: (page: ActivePage) => void;
}

function formatSpeed(bytesPerSec: number): { value: string; unit: string } {
  if (bytesPerSec >= 1024 * 1024) {
    return {
      value: (bytesPerSec / (1024 * 1024)).toFixed(1),
      unit: "МБ/с",
    };
  }
  return {
    value: (bytesPerSec / 1024).toFixed(0),
    unit: "КБ/с",
  };
}

export const OverviewPage: React.FC<OverviewPageProps> = ({
  metrics,
  history,
  processes,
  onNavigate,
}) => {
  const rxSpeed =
    metrics?.network.status === "Available" && metrics.network.value
      ? formatSpeed(metrics.network.value.rx_bytes_per_sec)
      : null;

  const txSpeed =
    metrics?.network.status === "Available" && metrics.network.value
      ? formatSpeed(metrics.network.value.tx_bytes_per_sec)
      : null;

  const freeBytes = metrics?.disk.value?.free_bytes;
  const freeGb = freeBytes !== undefined ? freeBytes / (1024 * 1024 * 1024) : null;
  const formattedDiskFree =
    freeGb !== null
      ? freeGb < 10
        ? freeGb.toFixed(1)
        : freeGb.toFixed(0)
      : "—";

  const formattedTime =
    metrics?.cpu.status === "Available"
      ? new Date(metrics.timestamp_utc).toLocaleTimeString("ru-RU", {
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
        })
      : "Ожидание данных";

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
          <strong
            className="text-txt cursor-help"
            title={metrics?.timestamp_utc || undefined}
          >
            {formattedTime}
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
          subText={
            metrics?.cpu.value
              ? `${metrics.cpu.value.core_count} ядер ${
                  metrics.cpu.value.frequency_mhz
                    ? `· ${metrics.cpu.value.frequency_mhz} МГц`
                    : ""
                }`
              : undefined
          }
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
          formattedValue={formattedDiskFree}
          unit="ГБ"
          subText={
            metrics?.disk.value
              ? `Диск ${metrics.disk.value.mount_point} (всего ${(
                  metrics.disk.value.total_bytes /
                  (1024 * 1024 * 1024)
                ).toFixed(0)} ГБ)`
              : undefined
          }
          progressPercent={metrics?.disk.value?.usage_percent}
        />

        <MetricCard
          title="Сетевая активность"
          tag="NET"
          icon={<Network className="w-4 h-4" />}
          field={metrics?.network || { status: "NotMeasured" }}
          formattedValue={rxSpeed ? rxSpeed.value : "—"}
          unit={rxSpeed ? rxSpeed.unit : "КБ/с"}
          subText={
            metrics?.network.value?.interface_name
              ? `Адаптер: ${metrics.network.value.interface_name}`
              : undefined
          }
        />
      </div>

      {/* Main Grid: Live Chart + Cleanup Action */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2">
          <ActivityChart history={history} />
        </div>
        <div>
          <CleanupCard onOpenCleanup={() => onNavigate("cleanup")} />
        </div>
      </div>

      {/* Bottom Grid: Process Preview + Network details */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2">
          <ProcessMiniTable
            processes={processes}
            onOpenProcesses={() => onNavigate("processes")}
          />
        </div>

        <article className="bg-surface border border-line rounded-xl p-5 flex flex-col justify-between">
          <div className="flex items-center justify-between mb-3">
            <div>
              <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
                Адаптеры
              </span>
              <h3 className="font-heading text-base font-semibold text-txt">
                Сетевой трафик
              </h3>
            </div>
            <Wifi className="w-4 h-4 text-accent" />
          </div>

          <div className="space-y-3 my-2 font-mono text-xs">
            <div className="flex items-center justify-between p-2.5 rounded-lg bg-shell border border-line">
              <span className="text-muted">↓ Приём (скорость)</span>
              <strong className="text-txt">
                {rxSpeed ? `${rxSpeed.value} ${rxSpeed.unit}` : "Ожидание"}
              </strong>
            </div>
            <div className="flex items-center justify-between p-2.5 rounded-lg bg-shell border border-line">
              <span className="text-muted">↑ Передача (скорость)</span>
              <strong className="text-txt">
                {txSpeed ? `${txSpeed.value} ${txSpeed.unit}` : "Ожидание"}
              </strong>
            </div>
          </div>

          <p className="text-[11px] text-dim text-center mt-2 font-mono">
            {metrics?.network.value?.interface_name || "Все сетевые интерфейсы"}
          </p>
        </article>
      </div>
    </div>
  );
};
