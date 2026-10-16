// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { App } from "./App";
import * as ipc from "./lib/ipc";

vi.mock("./lib/ipc", () => ({
  fetchAppInfo: vi.fn().mockResolvedValue({ version: "test", settings_persistent: true }),
  fetchAppSettings: vi.fn().mockResolvedValue({ is_monitoring_paused: false, refresh_interval_ms: 2000 }),
  fetchMetricSnapshot: vi.fn().mockResolvedValue({ timestamp_utc: "2026-10-16T00:00:00Z" }),
  fetchProcessList: vi.fn(), subscribeToMetrics: vi.fn().mockResolvedValue(() => {}),
  saveRefreshInterval: vi.fn(), togglePauseMetrics: vi.fn(), isTauriEnvironment: () => false,
}));
vi.mock("./pages/OverviewPage", () => ({ OverviewPage: () => <p>Обзор доступен</p> }));
vi.mock("./pages/CleanupPage", () => ({ CleanupPage: () => <p>Выбор файлов сохранён</p> }));
vi.mock("./pages/DiskPage", () => ({ DiskPage: () => null }));
vi.mock("./pages/ProcessesPage", () => ({ ProcessesPage: () => null }));
vi.mock("./pages/ServicePage", () => ({ ServicePage: () => null }));
vi.mock("./pages/JournalPage", () => ({ JournalPage: () => null }));
vi.mock("./pages/SettingsPage", () => ({ SettingsPage: () => null }));
vi.mock("./pages/AssistantPage", () => ({ AssistantPage: () => null }));

let container: HTMLDivElement, root: Root;
beforeEach(() => {
  vi.useFakeTimers(); vi.clearAllMocks(); vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.mocked(ipc.fetchProcessList).mockResolvedValue({ processes: [], timestamp_utc: "2026-10-16T00:00:00Z" });
  container = document.createElement("div"); document.body.append(container); root = createRoot(container);
});
afterEach(async () => { await act(async () => root.unmount()); container.remove(); vi.useRealTimers(); vi.unstubAllGlobals(); });

it("preserves the active page on a background failure and recovers on the next update", async () => {
  await act(async () => root.render(<App/>));
  const cleanup = [...container.querySelectorAll("nav button")].find(b => b.textContent === "Очистка") as HTMLButtonElement;
  await act(async () => cleanup.click());
  vi.mocked(ipc.fetchProcessList).mockRejectedValueOnce(new Error("Источник недоступен"));
  await act(async () => vi.advanceTimersByTimeAsync(10000));
  expect(container.textContent).toContain("Источник недоступен");
  expect(container.textContent).toContain("Выбор файлов сохранён");
  expect(container.textContent).not.toContain("Ошибка инициализации");
  await act(async () => vi.advanceTimersByTimeAsync(10000));
  expect(container.textContent).not.toContain("Источник недоступен");
  expect(container.textContent).toContain("Выбор файлов сохранён");
});
