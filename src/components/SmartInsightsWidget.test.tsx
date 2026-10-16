import { renderToStaticMarkup } from "react-dom/server";
import { describe, it, expect, vi } from "vitest";
import { SmartInsightsWidget } from "./SmartInsightsWidget";
import { SmartInsight } from "../types";

describe("SmartInsightsWidget", () => {
  const mockOptimalInsight: SmartInsight[] = [
    {
      id: "all_systems_optimal",
      title: "Система в оптимальном состоянии",
      description: "Свободное место в норме, системная Корзина не переполнена, параметры электропитания сбалансированы.",
      category: "system",
      level: "info",
      estimatedSavings: null,
      actionLabel: "Центр обслуживания",
      targetPage: "service",
      targetTab: null,
    },
  ];

  const mockMultipleInsights: SmartInsight[] = [
    {
      id: "power_battery_quiet",
      title: "Работа от аккумулятора: тихий режим",
      description: "Отключение агрессивного турбобуста (лимит 99%) снижает нагрев процессора на 5–15°C и продлевает время автономной работы ноутбука.",
      category: "power",
      level: "tip",
      estimatedSavings: "+20–35% к автономности",
      actionLabel: "Настроить питание",
      targetPage: "service",
      targetTab: "diagnostics",
    },
    {
      id: "recycle_bin_full",
      title: "Накопление файлов в Корзине",
      description: "В системной Корзине Windows находится 42 удалённых файлов объёмом 2.4 ГБ.",
      category: "recycle_bin",
      level: "warning",
      estimatedSavings: "2.4 ГБ",
      actionLabel: "Очистить Корзину",
      targetPage: "service",
      targetTab: "dev_recycle",
    },
  ];

  it("renders optimal system banner when all systems are healthy", () => {
    const onNavigate = vi.fn();
    const html = renderToStaticMarkup(
      <SmartInsightsWidget
        initialInsights={mockOptimalInsight}
        onNavigate={onNavigate}
      />
    );

    expect(html).toContain("Интеллектуальные рекомендации");
    expect(html).toContain("Система в оптимальном состоянии");
    expect(html).toContain("Свободное место в норме");
    expect(html).toContain("Центр обслуживания");
  });

  it("renders multiple recommendations with category badges and savings", () => {
    const onNavigate = vi.fn();
    const html = renderToStaticMarkup(
      <SmartInsightsWidget
        initialInsights={mockMultipleInsights}
        onNavigate={onNavigate}
      />
    );

    expect(html).toContain("Работа от аккумулятора: тихий режим");
    expect(html).toContain("Совет");
    expect(html).toContain("+20–35% к автономности");
    expect(html).toContain("Настроить питание");

    expect(html).toContain("Накопление файлов в Корзине");
    expect(html).toContain("Внимание");
    expect(html).toContain("2.4 ГБ");
    expect(html).toContain("Очистить Корзину");
  });
});
