import React from "react";
import { ProcessItem } from "../types";
import { ArrowRight, Cpu } from "lucide-react";

interface ProcessMiniTableProps {
  processes: ProcessItem[];
  onOpenProcesses: () => void;
}

export const ProcessMiniTable: React.FC<ProcessMiniTableProps> = ({
  processes,
  onOpenProcesses,
}) => {
  const topProcesses = processes.slice(0, 4);

  return (
    <article className="bg-surface border border-line rounded-xl p-5 flex flex-col justify-between">
      <div className="flex items-center justify-between mb-3">
        <div>
          <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
            Мониторинг
          </span>
          <h3 className="font-heading text-base font-semibold text-txt">
            Топ процессов по CPU
          </h3>
        </div>
        <button
          onClick={onOpenProcesses}
          className="flex items-center gap-1 text-xs text-accent hover:underline font-medium transition-all"
        >
          <span>Весь список ({processes.length})</span>
          <ArrowRight className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Table Head */}
      <div className="grid grid-cols-12 text-[10px] font-mono text-dim pb-2 border-b border-line px-2">
        <span className="col-span-6 font-medium">ПРОЦЕСС</span>
        <span className="col-span-2 text-right">PID</span>
        <span className="col-span-2 text-right">CPU</span>
        <span className="col-span-2 text-right">ПАМЯТЬ</span>
      </div>

      {/* Content */}
      {topProcesses.length === 0 ? (
        <div className="py-8 flex flex-col items-center justify-center text-center">
          <div className="w-8 h-8 rounded-full bg-shell border border-line flex items-center justify-center text-dim mb-2">
            <Cpu className="w-4 h-4" />
          </div>
          <strong className="text-xs font-medium text-txt">Нет данных о процессах</strong>
          <span className="text-[11px] text-muted mt-0.5">
            Данные читаются без административных привилегий
          </span>
        </div>
      ) : (
        <div className="divide-y divide-line/40 font-mono text-xs my-1">
          {topProcesses.map((proc) => {
            const cpuVal = proc.cpu_percent.status === "Available" ? proc.cpu_percent.value : undefined;
            const memBytes = proc.memory_bytes.status === "Available" ? proc.memory_bytes.value : undefined;
            const memMb =
              memBytes !== undefined ? (memBytes / (1024 * 1024)).toFixed(0) : "—";

            return (
              <div
                key={proc.pid}
                className="grid grid-cols-12 items-center px-2 py-2 hover:bg-shell/50 rounded transition-colors"
              >
                <div className="col-span-6 truncate pr-2">
                  <span className="text-txt font-medium">{proc.name}</span>
                </div>
                <div className="col-span-2 text-right text-dim">{proc.pid}</div>
                <div className="col-span-2 text-right text-accent font-semibold">
                  {cpuVal !== undefined ? `${cpuVal.toFixed(1)}%` : "—"}
                </div>
                <div className="col-span-2 text-right text-muted">{memMb} МБ</div>
              </div>
            );
          })}
        </div>
      )}
    </article>
  );
};
