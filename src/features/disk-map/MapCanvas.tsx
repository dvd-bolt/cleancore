import { useEffect, useMemo, useRef, useState } from "react";
import type { TreemapNode } from "../../types";
import { extensionColor, hitTest, layoutTree, type Tile } from "./layout";

export const bytes = (n: number) => {
  if (!n) return "0 Б";
  const units = ["Б", "КиБ", "МиБ", "ГиБ", "ТиБ"];
  const i = Math.min(4, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** i).toLocaleString("ru-RU", { maximumFractionDigits: i ? 2 : 0 })} ${units[i]}`;
};

export function MapCanvas({ root, selected, filter, onSelect, onOpen, onContext }: {
  root: TreemapNode; selected: string | null; filter: string | null;
  onSelect: (node: TreemapNode) => void; onOpen: (node: TreemapNode) => void;
  onContext: (node: TreemapNode, x: number, y: number) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const background = useRef<HTMLCanvasElement | null>(null);
  const [size, setSize] = useState({ w: 960, h: 480 });
  const [hover, setHover] = useState<{ tile: Tile; x: number; y: number } | null>(null);
  const tiles = useMemo(() => layoutTree(root, size.w, size.h), [root, size]);
  useEffect(() => {
    setHover(null);
    const element = host.current;
    if (!element) return;
    const measure = () => setSize({ w: Math.max(1, element.clientWidth), h: Math.max(1, element.clientHeight) });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [root]);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const ratio = Math.min(window.devicePixelRatio || 1, 2);
    element.width = Math.ceil(size.w * ratio); element.height = Math.ceil(size.h * ratio);
    const ctx = element.getContext("2d", { alpha: false });
    if (!ctx) return;
    ctx.scale(ratio, ratio);
    ctx.fillStyle = "#171819"; ctx.fillRect(0, 0, size.w, size.h);
    ctx.font = "12px system-ui, sans-serif";
    for (const tile of tiles) {
      const { x, y, w, h, node, header } = tile;
      const folder = node.is_dir && node.children.length > 0;
      const color = node.is_aggregate ? "#52565c" : node.is_dir ? "#626873" : extensionColor(node.extension);
      const dim = filter !== null && !node.is_dir && node.extension !== filter;
      ctx.globalAlpha = dim ? 0.19 : 1;
      ctx.fillStyle = folder ? "#303236" : color;
      ctx.fillRect(x, y, w, h);
      if (!folder && w > 3 && h > 3) {
        const glow = ctx.createRadialGradient(x + w * .42, y + h * .38, 0, x + w / 2, y + h / 2, Math.max(w, h) * .7);
        glow.addColorStop(0, "rgba(255,255,255,.23)"); glow.addColorStop(.45, "rgba(0,0,0,0)"); glow.addColorStop(1, "rgba(0,0,0,.65)");
        ctx.fillStyle = glow; ctx.fillRect(x + .5, y + .5, Math.max(0, w - 1), Math.max(0, h - 1));
      }
      ctx.strokeStyle = "rgba(10,12,15,.65)"; ctx.lineWidth = .65;
      ctx.strokeRect(x + .3, y + .3, Math.max(0, w - .6), Math.max(0, h - .6));
      if (w > 65 && (header || (!folder && h > 22))) {
        ctx.save(); ctx.beginPath(); ctx.rect(x + 3, y, w - 6, header || h); ctx.clip();
        ctx.fillStyle = "#f4f4f5"; ctx.shadowColor = "#000"; ctx.shadowBlur = 3;
        const label = `${node.name} (${bytes(node.size_bytes)})`;
        let text = label;
        while (text.length > 1 && ctx.measureText(text).width > w - 9) text = text.slice(0, -2);
        if (text !== label) text += "…";
        ctx.fillText(text, x + 4, y + 13); ctx.restore();
      }
    }
    ctx.globalAlpha = 1;
    const cached = document.createElement("canvas");
    cached.width = element.width; cached.height = element.height;
    cached.getContext("2d")?.drawImage(element, 0, 0);
    background.current = cached;
  }, [tiles, size, filter]);
  useEffect(() => {
    const element = canvas.current;
    const cached = background.current;
    const ctx = element?.getContext("2d");
    if (!ctx || !cached) return;
    const ratio = Math.min(window.devicePixelRatio || 1, 2);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.drawImage(cached, 0, 0);
    ctx.scale(ratio, ratio);
    for (const id of [hover?.tile.node.id, selected]) {
      const tile = tiles.find(t => t.node.id === id);
      if (!tile) continue;
      ctx.strokeStyle = id === selected ? "#fff" : "#d4d7df"; ctx.lineWidth = id === selected ? 2 : 1;
      ctx.strokeRect(tile.x + 1, tile.y + 1, Math.max(0, tile.w - 2), Math.max(0, tile.h - 2));
    }
  }, [tiles, size, filter, selected, hover?.tile.node.id]);
  const point = (e: React.MouseEvent) => {
    const r = e.currentTarget.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  return <div ref={host} className="relative w-full h-full min-h-0 overflow-hidden bg-[#171819]">
    <canvas ref={canvas} className="block w-full h-full outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-white" tabIndex={0}
      role="group" aria-label="Карта файлов. Стрелки выбирают объекты, Enter открывает папку."
      onMouseMove={e => { const p = point(e); const tile = hitTest(tiles, p.x, p.y); setHover(tile ? { tile, ...p } : null); }}
      onMouseLeave={() => setHover(null)}
      onClick={e => { const p = point(e); const tile = hitTest(tiles, p.x, p.y); if (tile) onSelect(tile.node); }}
      onDoubleClick={e => { const p = point(e); const tile = hitTest(tiles, p.x, p.y); if (tile?.node.is_dir) onOpen(tile.node); }}
      onContextMenu={e => { e.preventDefault(); const p = point(e); const tile = hitTest(tiles, p.x, p.y); if (tile) { onSelect(tile.node); onContext(tile.node, p.x, p.y); } }}
      onKeyDown={e => {
        if (e.key === "Enter") { const tile = tiles.find(t => t.node.id === selected); if (tile?.node.is_dir) onOpen(tile.node); return; }
        if (!["ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp"].includes(e.key)) return;
        e.preventDefault(); const targets = tiles.filter(t => t.w > 4 && t.h > 4);
        const current = targets.findIndex(t => t.node.id === selected);
        const next = (current + (["ArrowRight", "ArrowDown"].includes(e.key) ? 1 : -1) + targets.length) % targets.length;
        if (targets[next]) onSelect(targets[next].node);
      }} />
    {root.size_bytes === 0 && <div className="absolute inset-0 flex items-center justify-center text-sm text-muted pointer-events-none">Нет данных с ненулевой площадью в этом режиме</div>}
    {hover && <div role="tooltip" className="absolute z-10 pointer-events-none rounded-lg border border-white/20 bg-[#171819]/95 p-3 text-xs text-white shadow-xl max-w-[min(380px,90%)] max-h-40 overflow-hidden" style={{ left: Math.min(Math.max(4, hover.x + 14), Math.max(4, size.w - 390)), top: hover.y > size.h - 130 ? Math.max(4, hover.y - 115) : hover.y + 14 }}>
      <div className="break-all font-medium">{hover.tile.node.path}</div>
      <div className="mt-1">{bytes(hover.tile.node.size_bytes)} · {root.size_bytes ? (hover.tile.node.size_bytes / root.size_bytes * 100).toFixed(2) : 0}% папки</div>
      {hover.tile.node.is_aggregate && <div className="mt-1 text-neutral-400">Группа мелких объектов; весь размер сохранён.</div>}
    </div>}
  </div>;
}
