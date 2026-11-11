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
    // 五行石转换的两种石必须在刷新范围内（精炼页依赖它们定价）
    expect(ids).toContain("5_24423");
    expect(ids).toContain("5_24425");
    expect(ids).toContain("5_24426");
  });

  it("支持 limit 截断；无配方时只刷五行石，全空退回物品列表", () => {
    const catalog = mockCatalog();
    expect(collectRefreshWupinIds(catalog, 3)).toHaveLength(3);
    // 没有配方：五行石转换的两种石仍然要刷新
    const noRecipe = { ...catalog, peifang: [] };
    expect(collectRefreshWupinIds(noRecipe, 5)).toEqual(["5_24423", "5_24425", "5_24426"]);
    // 配方和五行石都没有：退回目录物品
    const empty = { ...catalog, peifang: [], wuxingshi: [] };
    expect(collectRefreshWupinIds(empty, 5)).toEqual(catalog.wupin.slice(0, 5).map((wupin) => wupin.id));
  });
});
