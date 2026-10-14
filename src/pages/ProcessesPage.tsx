import React, { useState } from "react";
import { Search, ArrowUpDown, Cpu } from "lucide-react";

export const ProcessesPage: React.FC = () => {
  const [searchTerm, setSearchTerm] = useState("");
  const [sortDesc, setSortDesc] = useState(true);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="pb-2 border-b border-line/40">
        <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
          Мониторинг
        </span>
        <h2 className="font-heading text-2xl font-bold text-txt">
          Процессы системы
        </h2>
        <p className="text-xs text-muted mt-1">
          Понятная таблица запущенных программ с идентификаторами и потреблением ресурсов.
        </p>
      </div>

      {/* Filter and Search Bar */}
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="relative flex-1 min-w-[240px] max-w-md">
          <Search className="w-4 h-4 text-dim absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            type="search"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Поиск по названию или PID..."
            className="w-full bg-shell border border-line rounded-lg pl-9 pr-4 py-2 text-xs text-txt placeholder:text-dim focus:outline-none focus:border-accent transition-all font-mono"
          />
        </div>

        <button
          onClick={() => setSortDesc(!sortDesc)}
          className="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface border border-line text-xs font-mono text-muted hover:text-txt transition-all"
        >
          <ArrowUpDown className="w-3.5 h-3.5 text-accent" />
          <span>Сортировка по CPU ({sortDesc ? "по убыванию" : "по возрастанию"})</span>
        </button>
      </div>

      {/* Table Container */}
      <div className="bg-surface border border-line rounded-xl overflow-hidden">
        {/* Table Head */}
        <div className="grid grid-cols-12 text-[11px] font-mono text-dim bg-shell/80 px-4 py-3 border-b border-line">
          <span className="col-span-6 font-medium">НАЗВАНИЕ ПРОЦЕССА</span>
          <span className="col-span-2 text-right">PID</span>
          <span className="col-span-2 text-right">CPU %</span>
          <span className="col-span-2 text-right">ПАМЯТЬ</span>
        </div>

        {/* Empty State on Stage 1 */}
        <div className="py-20 flex flex-col items-center justify-center text-center px-4">
          <div className="w-12 h-12 rounded-full bg-shell border border-line flex items-center justify-center text-dim mb-3">
            <Cpu className="w-6 h-6" />
          </div>
          <h3 className="font-heading text-base font-semibold text-txt">
            Нет данных о процессах
          </h3>
          <p className="text-xs text-muted max-w-md mt-1 leading-relaxed">
            Сборщик процессов будет подключён в модуле 2.2 дорожной карты. Данные выводятся строго без имитации и без административных привилегий.
          </p>
        </div>
      </div>
    </div>
  );
};
