import React from "react";
import { ActivePage } from "../types";
import { 
  LayoutDashboard, 
  Cpu, 
  HardDrive,
  Trash2, 
  ScrollText, 
  Settings, 
  Pause, 
  Play, 
  ShieldCheck,
  Wrench
} from "lucide-react";

interface HeaderProps {
  activePage: ActivePage;
  onPageChange: (page: ActivePage) => void;
  isPaused: boolean;
  onTogglePause: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  activePage,
  onPageChange,
  isPaused,
  onTogglePause,
}) => {
  const navItems: { id: ActivePage; label: string; num: string; icon: React.ReactNode }[] = [
    { id: "overview", label: "Обзор", num: "01", icon: <LayoutDashboard className="w-4 h-4" /> },
    { id: "processes", label: "Процессы", num: "02", icon: <Cpu className="w-4 h-4" /> },
    { id: "disk", label: "Диск", num: "03", icon: <HardDrive className="w-4 h-4" /> },
    { id: "cleanup", label: "Очистка", num: "04", icon: <Trash2 className="w-4 h-4" /> },
    { id: "service", label: "Сервис", num: "05", icon: <Wrench className="w-4 h-4" /> },
    { id: "journal", label: "Журнал", num: "06", icon: <ScrollText className="w-4 h-4" /> },
    { id: "settings", label: "Настройки", num: "07", icon: <Settings className="w-4 h-4" /> },
  ];

  return (
    <header className="bg-shell border-b border-line px-6 py-3 flex flex-wrap items-center justify-between gap-4 select-none">
      {/* Brand & Logo */}
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-lg bg-surface border border-line flex items-center justify-center text-accent">
          <ShieldCheck className="w-5 h-5 text-accent" />
        </div>
        <div>
          <div className="flex items-center gap-2">
            <span className="font-heading text-lg font-bold tracking-wide text-txt">
              CleanCore
            </span>
            <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface border border-line text-dim">
              STUDIO PEARL
            </span>
          </div>
          <p className="text-[11px] text-muted leading-none">
            Центр обслуживания системы
          </p>
        </div>
      </div>

      {/* Navigation tabs */}
      <nav className="flex items-center gap-1 bg-surface p-1 rounded-xl border border-line">
        {navItems.map((item) => {
          const isActive = activePage === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onPageChange(item.id)}
              className={`flex items-center gap-2 px-3.5 py-1.5 rounded-lg text-xs transition-all font-medium ${
                isActive
                  ? "bg-accent text-accent-ink shadow-sm font-semibold"
                  : "text-muted hover:text-txt hover:bg-surface-2"
              }`}
            >
              {item.icon}
              <span>{item.label}</span>
              <span
                className={`text-[9px] font-mono ml-0.5 ${
                  isActive ? "text-accent-ink/80" : "text-dim"
                }`}
              >
                {item.num}
              </span>
            </button>
          );
        })}
      </nav>

      {/* Controls & Mode */}
      <div className="flex items-center gap-3">
        <button
          onClick={onTogglePause}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs transition-all ${
            isPaused
              ? "bg-amber-950/40 border-amber-600/50 text-amber-200"
              : "bg-surface border-line text-muted hover:text-txt hover:bg-surface-2"
          }`}
          title={isPaused ? "Возобновить опрос" : "Приостановить опрос метрик"}
        >
          {isPaused ? <Play className="w-3.5 h-3.5" /> : <Pause className="w-3.5 h-3.5" />}
          <span>{isPaused ? "Возобновить" : "Пауза"}</span>
        </button>
      </div>
    </header>
  );
};
