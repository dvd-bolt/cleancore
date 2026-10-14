import React from "react";
import { ArrowRight, Cpu } from "lucide-react";

interface ProcessMiniTableProps {
  onOpenProcesses: () => void;
}

export const ProcessMiniTable: React.FC<ProcessMiniTableProps> = ({ onOpenProcesses }) => {
  return (
    <article className="bg-surface border border-line rounded-xl p-5 flex flex-col justify-between">
      <div className="flex items-center justify-between mb-3">
        <div>
          <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
            Мониторинг
          </span>
          <h3 className="font-heading text-base font-semibold text-txt">
            Процессы
          </h3>
        </div>
        <button
          onClick={onOpenProcesses}
          className="flex items-center gap-1 text-xs text-accent hover:underline font-medium"
        >
          <span>Открыть список</span>
          <ArrowRight className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Table Head */}
      <div className="grid grid-cols-4 text-[10px] font-mono text-dim pb-2 border-b border-line px-2">
        <span className="col-span-2">ПРОЦЕСС</span>
        <span className="text-right">CPU</span>
        <span className="text-right">ПАМЯТЬ</span>
      </div>

      {/* Empty State */}
      <div className="py-8 flex flex-col items-center justify-center text-center">
        <div className="w-8 h-8 rounded-full bg-shell border border-line flex items-center justify-center text-dim mb-2">
          <Cpu className="w-4 h-4" />
        </div>
        <strong className="text-xs font-medium text-txt">Список пока недоступен</strong>
        <span className="text-[11px] text-muted mt-0.5">
          Чтение процессов подключается на этапе 2
        </span>
      </div>
    </article>
  );
};
