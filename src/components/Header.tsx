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
  Wrench,
  MessageCircle,
  Battery,
  BatteryCharging,
  Zap,
} from "lucide-react";
import { PowerStatusSnapshot } from "../types";

interface HeaderProps {
  activePage: ActivePage;
  onPageChange: (page: ActivePage) => void;
  isPaused: boolean;
  onTogglePause: () => void;
  power?: PowerStatusSnapshot;
}

export const Header: React.FC<HeaderProps> = ({
  activePage,
  onPageChange,
  isPaused,
  onTogglePause,
  power,
}) => {
  const navItems: { id: ActivePage; label: string; icon: React.ReactNode }[] = [
    { id: "overview", label: "Обзор", icon: <LayoutDashboard className="w-4 h-4" /> },
    { id: "processes", label: "Процессы", icon: <Cpu className="w-4 h-4" /> },
    { id: "disk", label: "Диск", icon: <HardDrive className="w-4 h-4" /> },
    { id: "cleanup", label: "Очистка", icon: <Trash2 className="w-4 h-4" /> },
    { id: "service", label: "Обслуживание", icon: <Wrench className="w-4 h-4" /> },
    { id: "journal", label: "Журнал", icon: <ScrollText className="w-4 h-4" /> },
    { id: "settings", label: "Настройки", icon: <Settings className="w-4 h-4" /> },
    { id: "assistant", label: "Помощник", icon: <MessageCircle className="w-4 h-4" /> },
  ];

  return (
    <header className="app-header bg-shell border-b border-line px-5 py-3 grid grid-cols-[1fr_auto] items-center gap-3 select-none">
      {/* Brand & Logo */}
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-lg bg-surface border border-line flex items-center justify-center text-accent">
          <img src="/cleancore.svg" alt="" className="w-8 h-8" />
        </div>
        <div>
          <div className="flex items-center gap-2">
            <span className="font-heading text-lg font-bold tracking-wide text-txt">
              CleanCore
            </span>
          </div>
          <p className="text-[11px] text-muted leading-none">
            Центр обслуживания системы
          </p>
        </div>
      </div>

      {/* Navigation tabs */}
      <nav aria-label="Разделы приложения" className="order-3 col-span-2 flex overflow-x-auto items-center gap-1">
        {navItems.map((item) => {
          const isActive = activePage === item.id;
          return (
            <button
              key={item.id}
              aria-current={isActive ? "page" : undefined}
              onClick={() => onPageChange(item.id)}
              className={`shrink-0 flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-all font-medium ${
                isActive
                  ? "bg-accent text-accent-ink shadow-sm font-semibold"
                  : "text-muted hover:text-txt hover:bg-surface-2"
              }`}
            >
              {item.icon}
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>

      {/* Controls & Mode */}
      <div className="flex items-center gap-3">
        {power && (
          <div
            className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border text-xs font-mono select-none ${
              power.battery_percent != null && power.battery_percent < 20 && !power.is_ac_connected
                ? "bg-red-950/40 border-red-700/50 text-red-300"
                : "bg-surface border-line text-muted"
            }`}
            title={
              power.is_battery_present
                ? `${power.is_ac_connected ? "Питание от сети" : "Работа от аккумулятора"}${power.is_charging ? " (зарядка)" : ""}`
                : "Питание от сети (аккумулятор не обнаружен)"
            }
          >
            {power.is_battery_present ? (
              power.is_charging ? (
                <BatteryCharging className="w-3.5 h-3.5 text-accent" />
              ) : (
                <Battery className={`w-3.5 h-3.5 ${power.battery_percent != null && power.battery_percent < 20 ? "text-red-400" : "text-muted"}`} />
              )
            ) : (
              <Zap className="w-3.5 h-3.5 text-accent" />
            )}
            <span>
              {power.is_battery_present
                ? `${power.battery_percent != null ? `${Math.round(power.battery_percent)}%` : "АКБ"}${power.is_ac_connected ? " (Сеть)" : ""}`
                : "Сеть"}
            </span>
          </div>
        )}

        <button
          onClick={onTogglePause}
          aria-pressed={isPaused}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs transition-all ${
            isPaused
              ? "bg-amber-950/40 border-amber-600/50 text-amber-200"
              : "bg-surface border-line text-muted hover:text-txt hover:bg-surface-2"
          }`}
          title={isPaused ? "Возобновить опрос" : "Приостановить опрос метрик"}
        >
          {isPaused ? <Play className="w-3.5 h-3.5" /> : <Pause className="w-3.5 h-3.5" />}
          <span>{isPaused ? "Возобновить мониторинг" : "Пауза мониторинга"}</span>
        </button>
      </div>
    </header>
  );
};
