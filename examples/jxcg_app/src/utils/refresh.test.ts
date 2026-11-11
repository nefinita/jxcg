import { describe, expect, it } from "vitest";
import { collectRefreshWupinIds } from "./refresh";
import { mockCatalog } from "../api/mock";

describe("行情刷新范围", () => {
  it("优先配方材料并去重，产物补足", () => {
    const catalog = mockCatalog();
    const ids = collectRefreshWupinIds(catalog);
    expect(ids.length).toBeGreaterThan(0);
    expect(new Set(ids).size).toBe(ids.length);
    // 材料应排在产物之前：珠贝母/云芝是材料，云芝竹沥羹是产物。
    const firstMaterial = ids.indexOf("5_30853");
    const firstOutput = ids.indexOf("5_89346");
    expect(firstMaterial).toBeGreaterThanOrEqual(0);
    expect(firstOutput).toBeGreaterThanOrEqual(0);
    expect(firstMaterial).toBeLessThan(firstOutput);
  });

  it("支持 limit 截断，空配方退回物品列表", () => {
    const catalog = mockCatalog();
    expect(collectRefreshWupinIds(catalog, 3)).toHaveLength(3);
    const empty = { ...catalog, peifang: [] };
    const ids = collectRefreshWupinIds(empty, 5);
    expect(ids).toHaveLength(5);
    expect(ids).toEqual(catalog.wupin.slice(0, 5).map((wupin) => wupin.id));
  });
});
