import { describe, expect, it } from "vitest";
import { hitTest, layoutTree, partition } from "./layout";
import type { TreemapNode } from "../../types";
import type { DiskAnalysisReport } from "../../types";
import { summaryMap } from "./summary";

describe("treemap geometry", () => {
  it("never double counts a nested top file in an ordinary summary", () => {
    const report = {root_path:"C:\\",total_bytes:100,total_files:4,subdirs:[{path:"C:\\folder",name:"folder",size_bytes:70,is_dir:true,file_count:2}],largest_files:[{path:"C:\\folder\\nested.bin",name:"nested.bin",size_bytes:50,is_dir:false,file_count:1},{path:"C:\\root.txt",name:"root.txt",size_bytes:20,is_dir:false,file_count:1}]} as DiskAnalysisReport;
    const map=summaryMap(report);
    expect(map.root.children.map(n=>n.size_bytes)).toEqual([70,20,10]);
    expect(map.root.children.some(n=>n.name==="nested.bin")).toBe(false);
    expect(map.indexed).toBe(false);
  });
  it("preserves proportional area without overlap for skewed and large sets", () => {
    for (const weights of [[999999, 1, 1], [5, 4, 3, 2, 1], Array.from({ length: 12000 }, (_, i) => i + 1)]) {
      const tiles = partition(weights, n => n, { x: 0, y: 0, w: 1200, h: 500 });
      const total = weights.reduce((a, b) => a + b, 0);
      expect(tiles.reduce((sum, tile) => sum + tile.rect.w * tile.rect.h, 0)).toBeCloseTo(600000, 5);
      for (const t of tiles) {
        expect(t.rect.w * t.rect.h).toBeCloseTo(600000 * t.item / total, 5);
        expect(t.rect.x).toBeGreaterThanOrEqual(0); expect(t.rect.y).toBeGreaterThanOrEqual(0);
        expect(t.rect.x + t.rect.w).toBeLessThanOrEqual(1200.00001); expect(t.rect.y + t.rect.h).toBeLessThanOrEqual(500.00001);
      }
      if (tiles.length < 100) for (let i = 0; i < tiles.length; i++) for (let j = i + 1; j < tiles.length; j++) {
        const a = tiles[i].rect, b = tiles[j].rect;
        expect(Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)) * Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y))).toBeCloseTo(0, 6);
      }
    }
  });
  it("ignores zero/invalid weights and picks the deepest visible object", () => {
    expect(partition([0, -1, NaN], n => n, { x: 0, y: 0, w: 100, h: 100 })).toEqual([]);
    const leaf: TreemapNode = { id: "file", name: "file.bin", path: "C:/file.bin", size_bytes: 20, logical_bytes: 20, allocated_bytes: 20, is_dir: false, is_aggregate: false, extension: "bin", file_count: 1, children: [] };
    const root = { ...leaf, id: "root", name: "C:", path: "C:/", is_dir: true, children: [leaf] };
    const tiles = layoutTree(root, 100, 100);
    expect(hitTest(tiles, 50, 50)?.node.id).toBe("file");
    expect(hitTest(tiles, 50, 5)?.node.id).toBe("root");
    expect(hitTest(tiles, -1, 5)).toBeUndefined();
  });
});
