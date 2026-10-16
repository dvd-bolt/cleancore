import React from "react";
import { FieldValue } from "../types";

interface MetricCardProps {
  title: string;
  tag: string;
  icon: React.ReactNode;
  field: FieldValue<any>;
  formattedValue: string;
  unit: string;
  subText?: string;
  progressPercent?: number;
  progressLabel?: string;
}

export const MetricCard: React.FC<MetricCardProps> = ({
  title,
  tag,
  icon,
  field,
  formattedValue,
  unit,
  subText,
  progressPercent,
  progressLabel,
}) => {
  const getStatusLabel = () => {
    switch (field.status) {
      case "Available":
        return subText || "В норме";
      case "NotMeasured":
        return "Ожидание данных";
      case "AccessDenied":
        return "Доступ ограничен";
      case "Unavailable":
        return field.value ? String(field.value) : "Недоступно";
      default:
        return "Нет данных";
    }
  };

  const isAvailable = field.status === "Available";

  return (
    <article className="bg-surface border border-line rounded-xl p-4 flex flex-col justify-between hover:border-dim/60 transition-all">
      {/* Head */}
      <div className="flex items-center justify-between text-muted text-xs mb-2">
        <div className="flex items-center gap-2 text-txt">
          <span className="text-accent">{icon}</span>
          <span className="font-medium">{title}</span>
        </div>
        <span className="font-mono text-[10px] text-dim px-1.5 py-0.5 rounded bg-shell border border-line/60">
          {tag}
        </span>
      </div>

      {/* Main Value */}
      <div className="my-2">
        <div className="flex items-baseline gap-1">
          <span className="font-heading text-3xl font-bold tracking-tight text-txt">
            {isAvailable ? formattedValue : "—"}
          </span>
          <span className="text-xs font-mono text-muted">{unit}</span>
        </div>
      </div>

      {/* Foot & Progress */}
      <div className="mt-2 pt-2 border-t border-line/50 flex flex-col gap-1.5">
        <div className="flex items-center justify-between text-[11px]">
          <span className={`text-[11px] ${isAvailable ? "text-muted" : "text-dim"}`}>
            {getStatusLabel()}
          </span>
          {isAvailable && progressPercent !== undefined && (
            <span className="font-mono text-[10px] text-muted">{progressLabel ? `${progressLabel}: ` : ""}{progressPercent.toFixed(0)}%</span>
          )}
        </div>

        {/* Progress Bar */}
        <div className="h-1.5 w-full bg-shell rounded-full overflow-hidden border border-line/40">
          {isAvailable && progressPercent !== undefined ? (
            <div
              className="h-full bg-accent transition-all duration-500 rounded-full"
              style={{ width: `${Math.min(100, Math.max(0, progressPercent))}%` }}
            />
          ) : (
            <div className="h-full w-0 bg-dim/20" />
          )}
        </div>
      </div>
    </article>
  );
};
