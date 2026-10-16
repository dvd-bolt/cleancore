import type { TreemapNode } from "../../types";

export interface Rect { x: number; y: number; w: number; h: number }
export interface Tile extends Rect { node: TreemapNode; depth: number; header: number }

/** Balanced binary partition: linear area conservation, logarithmic partition depth. */
export function partition<T>(items: T[], weight: (item: T) => number, rect: Rect): { item: T; rect: Rect }[] {
  const valid = items.filter(item => Number.isFinite(weight(item)) && weight(item) > 0);
  const sums = [0];
  for (const item of valid) sums.push(sums[sums.length - 1] + weight(item));
  const result: { item: T; rect: Rect }[] = [];
  const split = (start: number, end: number, area: Rect) => {
    if (start >= end || area.w <= 0 || area.h <= 0) return;
    if (end - start === 1) { result.push({ item: valid[start], rect: area }); return; }
    const total = sums[end] - sums[start];
    const half = sums[start] + total / 2;
    let low = start + 1, high = end - 1;
    while (low < high) { const mid = (low + high) >>> 1; if (sums[mid] < half) low = mid + 1; else high = mid; }
    let cut = low;
    if (cut > start + 1 && Math.abs(sums[cut - 1] - half) < Math.abs(sums[cut] - half)) cut--;
    const fraction = (sums[cut] - sums[start]) / total;
    if (area.w >= area.h) {
      const w = area.w * fraction;
      split(start, cut, { ...area, w });
      split(cut, end, { ...area, x: area.x + w, w: area.w - w });
    } else {
      const h = area.h * fraction;
      split(start, cut, { ...area, h });
      split(cut, end, { ...area, y: area.y + h, h: area.h - h });
    }
  };
  split(0, valid.length, rect);
  return result;
}

export function layoutTree(root: TreemapNode, width: number, height: number): Tile[] {
  const tiles: Tile[] = [];
  const visit = (node: TreemapNode, rect: Rect, depth: number) => {
    const header = node.is_dir && rect.w > 70 && rect.h > 34 ? 18 : 0;
    tiles.push({ ...rect, node, depth, header });
    if (!node.children.length || rect.w < 5 || rect.h - header < 5) return;
    const inner = { x: rect.x + 1, y: rect.y + header + 1, w: Math.max(0, rect.w - 2), h: Math.max(0, rect.h - header - 2) };
    for (const child of partition(node.children, n => n.size_bytes, inner)) {
      if (child.rect.w * child.rect.h >= 0.5) visit(child.item, child.rect, depth + 1);
    }
  };
  if (width > 0 && height > 0 && root.size_bytes > 0) visit(root, { x: 0, y: 0, w: width, h: height }, 0);
  return tiles;
}

export function hitTest(tiles: Tile[], x: number, y: number): Tile | undefined {
  for (let i = tiles.length - 1; i >= 0; i--) {
    const r = tiles[i];
    if (x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h) return r;
  }
}

const PALETTE = ["#d97706", "#b4ad16", "#8b5cf6", "#0891b2", "#c026d3", "#16a34a", "#db2777", "#2563eb", "#d94816", "#089981", "#a855f7", "#b98d34"];
export function extensionColor(extension: string): string {
  if (!extension) return "#64748b";
  let hash = 0;
  for (let i = 0; i < extension.length; i++) hash = (hash * 31 + extension.charCodeAt(i)) >>> 0;
  return PALETTE[hash % PALETTE.length];
}
