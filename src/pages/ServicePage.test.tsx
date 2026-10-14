import { renderToStaticMarkup } from "react-dom/server";
import { describe, it, expect } from "vitest";
import {
  ServicePage,
  formatBytes,
  formatInstallDate,
} from "./ServicePage";
import {
  AntivirusStatus,
  DiagnosticsSummary,
  InstalledProgram,
  StartupItem,
} from "../types";

const mockPrograms: InstalledProgram[] = [
  {
    id: "HKLM_64_7Zip",
    display_name: "7-Zip 24.08 (x64)",
    publisher: "Igor Pavlov",
    display_version: "24.08",
    install_date: "20260115",
    estimated_size_bytes: 5242880,
    uninstall_string: "C:\\Program Files\\7-Zip\\Uninstall.exe",
    quiet_uninstall_string: null,
    install_location: "C:\\Program Files\\7-Zip",
    registry_hive: "HKLM",
    is_wow64: false,
    is_system_component: false,
  },
  {
    id: "HKCU_64_VSCode",
    display_name: "Microsoft Visual Studio Code",
    publisher: "Microsoft Corporation",
    display_version: "1.95.0",
    install_date: "20260210",
    estimated_size_bytes: 380000000,
    uninstall_string: "C:\\Users\\User\\AppData\\Local\\Programs\\Microsoft VS Code\\unins000.exe",
    quiet_uninstall_string: null,
    install_location: "C:\\Users\\User\\AppData\\Local\\Programs\\Microsoft VS Code",
    registry_hive: "HKCU",
    is_wow64: false,
    is_system_component: false,
  },
  {
    id: "HKLM_32_Git",
    display_name: "Git version 2.47.0",
    publisher: "The Git Development Community",
    display_version: "2.47.0",
    install_date: "20260120",
    estimated_size_bytes: 320000000,
    uninstall_string: "C:\\Program Files\\Git\\unins000.exe",
    quiet_uninstall_string: null,
    install_location: "C:\\Program Files\\Git",
    registry_hive: "HKLM",
    is_wow64: true,
    is_system_component: false,
  },
];

const mockStartup: StartupItem[] = [
  {
    id: "HKCU_Run_OneDrive",
    name: "OneDrive",
    command: "\"C:\\Users\\User\\AppData\\Local\\Microsoft\\OneDrive\\OneDrive.exe\" /background",
    source: "registry_run",
    location: "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
    enabled: true,
    target_exists: true,
  },
  {
    id: "User_Startup_CleanCoreHelper",
    name: "CleanCoreHelper.lnk",
    command: "C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\CleanCoreHelper.lnk",
    source: "startup_folder_user",
    location: "C:\\Users\\User\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup",
    enabled: true,
    target_exists: false,
  },
];

const mockAntivirus: AntivirusStatus = {
  state: "protected",
  providers: [
    {
      name: "Windows Defender",
      product_exe: "C:\\Program Files\\Windows Defender\\MsMpEng.exe",
      reporting_exe: null,
      state_code: 397568,
      real_time_protection: true,
      definitions_up_to_date: true,
    },
  ],
  defender_service_running: true,
  message: "Защита активна: Windows Defender включен, сигнатуры актуальны.",
};

const mockDiagnostics: DiagnosticsSummary = {
  health: "healthy",
  cpu_usage_percent: 18.5,
  memory_usage_percent: 54,
  disk_free_bytes: 48500000000,
  disk_total_bytes: 256000000000,
  disk_usage_percent: 81,
  system_drive: "C:",
  alerts: [
    {
      id: "diag_disk_ok",
      level: "healthy",
      category: "disk",
      title: "Свободное место на системном диске",
      message: "На диске C: свободно 48.5 ГБ (19%).",
      recommendation: null,
    },
  ],
  recommendations: [
    "Система функционирует штатно, критических предупреждений нет.",
  ],
  timestamp_utc: "2026-10-14T12:00:00Z",
};

describe("ServicePage UI Component", () => {
  it("renders Service Center header with title and all 4 tab buttons", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialPrograms={mockPrograms}
        initialStartup={mockStartup}
        initialAntivirus={mockAntivirus}
        initialDiagnostics={mockDiagnostics}
      />
    );

    expect(html).toContain("Центр обслуживания");
    expect(html).toContain("v0.3 • МОДУЛИ 6.1–6.6");
    expect(html).toContain("1. Программы");
    expect(html).toContain("2. Автозагрузка");
    expect(html).toContain("3. Защита и здоровье");
    expect(html).toContain("4. Инструменты Windows");
    expect(html).toContain("Экспорт отчёта (CSV)");
  });

  it("renders Programs tab with software list and formatted metadata", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="programs"
        initialPrograms={mockPrograms}
      />
    );

    expect(html).toContain("7-Zip 24.08 (x64)");
    expect(html).toContain("Microsoft Visual Studio Code");
    expect(html).toContain("Git version 2.47.0");
    expect(html).toContain("Igor Pavlov");
    expect(html).toContain("Microsoft Corporation");
    expect(html).toContain("15.01.2026");
    expect(html).toContain("10.02.2026");
    expect(html).toContain("5 МБ");
    expect(html).toContain("Удалить");
  });

  it("filters programs by search query", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="programs"
        initialPrograms={mockPrograms}
        initialSearch="VSCode"
      />
    );

    expect(html).toContain("Microsoft Visual Studio Code");
    expect(html).not.toContain("7-Zip 24.08 (x64)");
    expect(html).not.toContain("Git version 2.47.0");
  });

  it("filters programs by 32-bit architecture", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="programs"
        initialPrograms={mockPrograms}
        initialFilterArch="32"
      />
    );

    expect(html).toContain("Git version 2.47.0");
    expect(html).not.toContain("7-Zip 24.08 (x64)");
    expect(html).not.toContain("Microsoft Visual Studio Code");
  });

  it("renders Startup tab with read-only warning and item target indicators", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="startup"
        initialStartup={mockStartup}
      />
    );

    expect(html).toContain("Безопасный аудит автозапуска (Только чтение)");
    expect(html).toContain("OneDrive");
    expect(html).toContain("CleanCoreHelper.lnk");
    expect(html).toContain("Файл найден");
    expect(html).toContain("Файл отсутствует");
    expect(html).toContain("Реестр Run");
    expect(html).toContain("Папка пользователя");
  });

  it("renders Protection & Health tab with Defender and system diagnostics", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="diagnostics"
        initialAntivirus={mockAntivirus}
        initialDiagnostics={mockDiagnostics}
      />
    );

    expect(html).toContain("Антивирусная защита Windows");
    expect(html).toContain("Защита активна");
    expect(html).toContain("Windows Defender");
    expect(html).toContain("Процессор (CPU)");
    expect(html).toContain("Память (RAM)");
    expect(html).toContain("Системный диск (C:)");
    expect(html).toContain("Рекомендации по оптимизации и здоровью системы");
    expect(html).toContain("Система функционирует штатно, критических предупреждений нет.");
  });

  it("renders Defender limited access state honestly without UAC escalation", () => {
    const limitedAv: AntivirusStatus = {
      state: "limited_access",
      providers: [],
      defender_service_running: null,
      message: "Статус антивируса недоступен в непривилегированном режиме.",
    };

    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="diagnostics"
        initialAntivirus={limitedAv}
        initialDiagnostics={mockDiagnostics}
      />
    );

    expect(html).toContain("Ограниченный доступ");
    expect(html).toContain("Статус антивируса недоступен в непривилегированном режиме.");
  });

  it("renders Windows Tools tab with all 5 native whitelisted utilities", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="tools"
      />
    );

    expect(html).toContain("Очистка диска");
    expect(html).toContain("cleanmgr.exe");
    expect(html).toContain("Дефрагментация и оптимизация");
    expect(html).toContain("dfrgui.exe");
    expect(html).toContain("Монитор ресурсов");
    expect(html).toContain("resmon.exe");
    expect(html).toContain("Диспетчер задач");
    expect(html).toContain("Taskmgr.exe");
    expect(html).toContain("Программы и компоненты");
    expect(html).toContain("control.exe appwiz.cpl");
    expect(html).toContain("Strict Whitelist");
  });

  it("renders safety confirmation dialog for uninstaller execution", () => {
    const targetProg = mockPrograms[0];
    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="programs"
        initialPrograms={mockPrograms}
        initialUninstallerProg={targetProg}
      />
    );

    expect(html).toContain("Запуск официального деинсталлятора");
    expect(html).toContain("7-Zip 24.08 (x64)");
    expect(html).toContain("C:\\Program Files\\7-Zip\\Uninstall.exe");
    expect(html).toContain("не производит принудительное удаление файлов");
    expect(html).toContain("Я подтверждаю запуск штатного деинсталлятора этой программы");
    // Button should be disabled when not acknowledged
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>[\s\S]*?Запустить деинсталлятор/);
  });

  it("enables uninstaller launch button when acknowledged", () => {
    const targetProg = mockPrograms[0];
    const html = renderToStaticMarkup(
      <ServicePage
        initialTab="programs"
        initialPrograms={mockPrograms}
        initialUninstallerProg={targetProg}
        initialUninstallerAcknowledged={true}
      />
    );

    expect(html).toContain("Запустить деинсталлятор");
    // Not disabled
    expect(html).not.toMatch(/<button[^>]*disabled=""[^>]*>[\s\S]*?Запустить деинсталлятор/);
  });

  it("renders CSV export dialog with RFC 4180 and CWE-1236 notice", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialPrograms={mockPrograms}
        initialStartup={mockStartup}
        initialAntivirus={mockAntivirus}
        initialDiagnostics={mockDiagnostics}
        initialExportModalOpen={true}
      />
    );

    expect(html).toContain("Экспорт отчёта обслуживания (CSV)");
    expect(html).toContain("Установленные программы");
    expect(html).toContain("Элементы автозагрузки");
    expect(html).toContain("Диагностика здоровья и статус защиты Defender");
    expect(html).toContain("CWE-1236");
    expect(html).toContain("Сформировать отчёт");
  });

  it("displays tool launch feedback message when available", () => {
    const html = renderToStaticMarkup(
      <ServicePage
        initialFeedbackMessage={{
          type: "success",
          text: "Штатная системная утилита 'cleanmgr' успешно запущена (PID: 7788).",
        }}
      />
    );

    expect(html).toContain("Штатная системная утилита &#x27;cleanmgr&#x27; успешно запущена (PID: 7788).");
  });

  it("formats bytes accurately", () => {
    expect(formatBytes(null)).toBe("—");
    expect(formatBytes(undefined)).toBe("—");
    expect(formatBytes(0)).toBe("0 Б");
    expect(formatBytes(1024)).toBe("1.0 КБ");
    expect(formatBytes(1048576)).toBe("1.0 МБ");
    expect(formatBytes(1073741824)).toBe("1.0 ГБ");
    expect(formatBytes(5242880)).toBe("5.0 МБ");
  });

  it("formats Windows Registry install dates accurately", () => {
    expect(formatInstallDate(null)).toBe("—");
    expect(formatInstallDate("")).toBe("—");
    expect(formatInstallDate("20260115")).toBe("15.01.2026");
    expect(formatInstallDate("20261231")).toBe("31.12.2026");
    expect(formatInstallDate("15.01.2026")).toBe("15.01.2026");
  });
});
