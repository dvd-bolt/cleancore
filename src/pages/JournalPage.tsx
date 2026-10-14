import React from "react";
import { ScrollText, Database } from "lucide-react";

export const JournalPage: React.FC = () => {
  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="pb-2 border-b border-line/40">
        <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
          Локальная история
        </span>
        <h2 className="font-heading text-2xl font-bold text-txt">
          Каждое действие. <span className="text-dim">Без недомолвок.</span>
        </h2>
        <p className="text-xs text-muted mt-1">
          Результаты очистки, пропущенные файлы и причины системных ошибок в локальной базе данных.
        </p>
      </div>

      {/* Main Journal Container */}
      <div className="bg-surface border border-line rounded-xl overflow-hidden p-8 flex flex-col items-center justify-center text-center">
        <div className="w-12 h-12 rounded-full bg-shell border border-line flex items-center justify-center text-dim mb-3">
          <ScrollText className="w-6 h-6" />
        </div>
        <h3 className="font-heading text-base font-semibold text-txt">
          История пока пуста
        </h3>
        <p className="text-xs text-muted max-w-md mt-1 leading-relaxed">
          Файловые операции ещё не выполнялись. Локальное хранилище журнала на базе SQLite будет подключено в модуле 4.4.
        </p>

        <div className="mt-6 flex items-center gap-2 text-xs font-mono text-dim bg-shell px-4 py-2 rounded-lg border border-line">
          <Database className="w-3.5 h-3.5 text-accent" />
          <span>База данных: cleaner-storage (SQLite локально)</span>
        </div>
      </div>
    </div>
  );
};
