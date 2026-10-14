import React from "react";
import { Shield, Monitor, HardDrive } from "lucide-react";
import { AppInfo } from "../types";

interface FooterProps {
  appInfo: AppInfo | null;
}

export const Footer: React.FC<FooterProps> = ({ appInfo }) => {
  return (
    <footer className="bg-shell border-t border-line px-6 py-2 flex flex-wrap items-center justify-between text-[11px] text-dim font-mono select-none">
      <div className="flex items-center gap-4">
        <span className="flex items-center gap-1.5 text-txt">
          <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
          <span>Локальный режим</span>
        </span>
        <span className="flex items-center gap-1.5">
          <Shield className="w-3.5 h-3.5 text-accent" />
          <span>Без телеметрии и сетевых запросов</span>
        </span>
      </div>

      <div className="flex items-center gap-4">
        <span className="flex items-center gap-1.5">
          <Monitor className="w-3.5 h-3.5" />
          <span>
            {appInfo?.is_admin ? "Права: Администратор" : "Обычные права пользователя"}
          </span>
        </span>
        <span className="flex items-center gap-1.5">
          <HardDrive className="w-3.5 h-3.5" />
          <span>v{appInfo?.version || "0.0.1"}</span>
        </span>
      </div>
    </footer>
  );
};
