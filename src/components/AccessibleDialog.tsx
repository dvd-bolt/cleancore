import { useRef, type ReactNode } from "react";
import * as Dialog from "@radix-ui/react-dialog";

export function AccessibleDialog({ title, onClose, busy = false, children }: {
  title: string; onClose: () => void; busy?: boolean; children: ReactNode;
}) {
  const returnFocus = useRef<HTMLElement | null>(typeof document === "undefined" ? null : document.activeElement as HTMLElement);
  return <Dialog.Root open onOpenChange={(open) => { if (!open && !busy) onClose(); }}>
    <Dialog.Content aria-describedby={undefined}
      onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }}
      onCloseAutoFocus={(event) => { event.preventDefault(); returnFocus.current?.focus(); }}
      className="fixed inset-0 z-50 bg-black/70 flex items-center justify-center p-4 overflow-y-auto">
      <Dialog.Title className="sr-only">{title}</Dialog.Title>
      {children}
    </Dialog.Content>
  </Dialog.Root>;
}
