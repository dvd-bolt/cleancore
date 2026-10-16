// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { AccessibleDialog } from "./AccessibleDialog";

it("names the modal, traps keyboard focus, closes on Escape and restores focus", async () => {
  const launcher = document.createElement("button");
  const host = document.createElement("div");
  document.body.append(launcher, host);
  launcher.focus();
  const close = vi.fn();
  const root = createRoot(host);
  await act(async () => { root.render(<AccessibleDialog title="Подтверждение" onClose={close}><button>Отмена</button><button>Продолжить</button></AccessibleDialog>); });
  const dialog = document.querySelector('[role="dialog"]')!;
  expect(document.getElementById(dialog.getAttribute("aria-labelledby")!)?.textContent).toBe("Подтверждение");
  expect(dialog.contains(document.activeElement)).toBe(true);
  const buttons = dialog.querySelectorAll("button");
  buttons[1].focus();
  await act(async () => { buttons[1].dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true })); });
  expect(document.activeElement).toBe(buttons[0]);
  await act(async () => { document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })); });
  expect(close).toHaveBeenCalledOnce();
  await act(async () => { root.unmount(); });
  await new Promise(resolve => setTimeout(resolve, 5));
  expect(document.activeElement).toBe(launcher);
  launcher.remove(); host.remove();
});
