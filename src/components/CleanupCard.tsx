import React from "react";
import { ArrowRight, ShieldCheck, Sparkles } from "lucide-react";

interface CleanupCardProps {
  onOpenCleanup: () => void;
}

export const CleanupCard: React.FC<CleanupCardProps> = ({ onOpenCleanup }) => {
  return (
    <article className="bg-surface border border-line rounded-xl p-5 flex flex-col justify-between relative overflow-hidden">
      {/* Decorative background glow */}
      <div className="absolute -right-10 -bottom-10 w-40 h-40 bg-accent-wash/30 rounded-full blur-2xl pointer-events-none" />

      <div>
        <div className="flex items-center justify-between mb-3">
          <span className="text-[10px] font-mono tracking-widest text-dim uppercase flex items-center gap-1.5">
            <Sparkles className="w-3 h-3 text-accent" />
            Освободи пространство
          </span>
          <span className="text-[10px] font-mono text-dim px-2 py-0.5 rounded bg-shell border border-line">
            STEP 01
          </span>
        </div>

        <h3 className="font-heading text-lg font-bold text-txt leading-snug mb-1">
          Сначала посмотреть. <br />
          Потом решить.
        </h3>

        <p className="text-xs text-muted leading-relaxed mb-4">
          Безопасный анализ временных файлов без риска: находите кандидатов без удаления.
        </p>
      </div>

      <div className="flex flex-col gap-3">
        <button
          onClick={onOpenCleanup}
          className="w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg bg-accent text-accent-ink font-semibold text-xs hover:opacity-90 transition-all shadow-sm"
        >
          <span>Открыть очистку</span>
          <ArrowRight className="w-4 h-4" />
        </button>

        <div className="flex items-center justify-center gap-1.5 text-[11px] text-dim">
          <ShieldCheck className="w-3.5 h-3.5 text-accent" />
          <span>Ничего не удаляется автоматически</span>
        </div>
      </div>
    </article>
  );
};
