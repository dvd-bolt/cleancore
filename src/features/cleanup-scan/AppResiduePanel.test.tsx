// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, afterEach, it, expect, vi } from "vitest";
import { AppResiduePanel } from "./AppResiduePanel";
import * as ipc from "../../lib/ipc";
vi.mock("../../lib/ipc", () => ({ inspectAppResidue: vi.fn() }));
let container: HTMLDivElement, root: Root;
beforeEach(() => { vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true); vi.clearAllMocks(); container = document.createElement("div"); document.body.append(container); root = createRoot(container); });
afterEach(async () => { await act(async () => root.unmount()); container.remove(); vi.unstubAllGlobals(); });
it("keeps the residue review read-only and distinguishes installed-app data and unknown size", async () => {
  vi.mocked(ipc.inspectAppResidue).mockResolvedValue({ app_name: "Owned fixture", still_registered: true, complete: false, inspected_entries: 10, warnings: ["Часть каталогов недоступна"], candidates: [{ path: "C:\\Users\\owned\\AppData\\Local\\<script>", reason: "Совпадает часть имени", confidence: "низкая", logical_bytes: null, files: 0, complete: false }] });
  await act(async () => root.render(<AppResiduePanel contextId="ctx" appName="Owned fixture" onDismiss={vi.fn()}/>));
  expect(ipc.inspectAppResidue).not.toHaveBeenCalled();
  await act(async () => container.querySelector("button")!.click());
  expect(ipc.inspectAppResidue).toHaveBeenCalledWith("ctx");
  expect(container.textContent).toContain("Программа ещё зарегистрирована");
  expect(container.textContent).toContain("Объём недоступен");
  expect(container.querySelector("script")).toBeNull();
  expect(Array.from(container.querySelectorAll("button")).some(b => /удалить/i.test(b.textContent ?? ""))).toBe(false);
});
