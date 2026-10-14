import React, { useState } from "react";
import { ShieldCheck, Info, FolderArchive, ArrowRight } from "lucide-react";

export const CleanupPage: React.FC = () => {
  const [showNotice, setShowNotice] = useState(false);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="pb-2 border-b border-line/40">
        <span className="text-[10px] font-mono tracking-widest text-dim uppercase">
          Безопасная очистка
        </span>
        <h2 className="font-heading text-2xl font-bold text-txt">
          Меньше лишнего. <span className="text-dim">Больше контроля.</span>
        </h2>
        <p className="text-xs text-muted mt-1">
          Вы выбираете файлы. Программа проверяет каждое действие перед отправкой в Корзину.
        </p>
      </div>

      {/* Steps Pipeline */}
      <div className="grid grid-cols-4 gap-2 text-xs font-mono select-none">
        {[
          { num: "01", label: "Анализ", active: true },
          { num: "02", label: "Просмотр", active: false },
          { num: "03", label: "Подтверждение", active: false },
          { num: "04", label: "Результат", active: false },
        ].map((step) => (
          <div
            key={step.num}
            className={`p-3 rounded-xl border flex items-center gap-2.5 transition-all ${
              step.active
                ? "bg-surface border-accent text-txt font-semibold"
                : "bg-shell/60 border-line text-dim"
            }`}
          >
            <span
              className={`text-[10px] px-1.5 py-0.5 rounded ${
                step.active ? "bg-accent text-accent-ink" : "bg-surface text-dim"
              }`}
            >
              {step.num}
            </span>
            <span>{step.label}</span>
          </div>
        ))}
      </div>

      {/* Category Card */}
      <article className="bg-surface border border-line rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-shell border border-line flex items-center justify-center text-accent">
            <FolderArchive className="w-6 h-6" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h3 className="font-heading text-base font-semibold text-txt">
                Временные файлы пользователя
              </h3>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-shell border border-line text-dim">
                TEMP / APPDATA
              </span>
            </div>
            <p className="text-xs text-muted mt-0.5">
              Файлы старше 7 дней. Символические ссылки, junctions и недоступные объекты автоматически исключаются.
            </p>
            <span className="inline-block mt-2 text-[11px] font-mono text-dim">
              Статус: Ещё не сканировалось
            </span>
          </div>
        </div>

        <button
          onClick={() => setShowNotice(true)}
          className="flex items-center gap-2 px-5 py-2.5 rounded-lg bg-accent text-accent-ink font-semibold text-xs hover:opacity-90 transition-all shadow-sm"
        >
          <span>Запустить сканирование</span>
          <ArrowRight className="w-4 h-4" />
        </button>
      </article>

      {/* Inline Notice */}
      {showNotice && (
        <div className="p-4 rounded-xl bg-shell border border-line flex items-start gap-3 text-xs text-muted animate-fadeIn">
          <Info className="w-4 h-4 text-accent shrink-0 mt-0.5" />
          <div>
            <strong className="text-txt block mb-0.5">Сканер находится в разработке</strong>
            <span>
              Модуль безопасного сканирования (3.1–3.6) и исполнитель (4.1–4.6) появятся на следующих этапах. Файлы на диске сейчас не сканируются и не изменяются.
            </span>
          </div>
        </div>
      )}

      {/* Safety Guarantee Banner */}
      <div className="p-5 rounded-xl bg-shell/50 border border-line/80 flex items-start gap-4">
        <div className="w-9 h-9 rounded-lg bg-surface border border-line flex items-center justify-center text-accent shrink-0">
          <ShieldCheck className="w-5 h-5 text-accent" />
        </div>
        <div>
          <h4 className="font-heading text-sm font-semibold text-txt">
            Гарантия контроля и безопасность путей
          </h4>
          <p className="text-xs text-muted mt-1 leading-relaxed">
            CleanCore никогда не удаляет файлы без вашего явного подтверждения. Каждая операция проверяется backend на подмену цели (TOCTOU), защищена от обхода каталогов (path traversal) и протоколируется в локальный журнал.
          </p>
        </div>
      </div>
    </div>
  );
};
