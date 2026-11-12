import { describe, expect, it } from "vitest";
import { collectRefreshWupinIds } from "./refresh";
import { mockCatalog } from "../api/mock";
import type { Catalog } from "../types";

describe("行情刷新范围（task-8：全量相关物品、不截断）", () => {
  it("覆盖五行石 + 全部配方材料 + 全部产物，去重且稳定排序", () => {
    const catalog = mockCatalog();
    const expected = new Set<string>();
    for (const zhuanhuan of catalog.wuxingshi) {
      expected.add(zhuanhuan.cong);
      expected.add(zhuanhuan.dao);
    }
    for (const peifang of catalog.peifang) {
      for (const cailiao of peifang.cailiao) expected.add(cailiao.wupin_id);
      for (const chuchan of peifang.chuchan) expected.add(chuchan.wupin_id);
    }
    const ids = collectRefreshWupinIds(catalog);
    expect(new Set(ids).size).toBe(ids.length); // 去重
    expect(ids.length).toBe(expected.size); // 全覆盖
    for (const id of expected) expect(ids).toContain(id);
    // 稳定顺序：五行石在最前；材料阶段先于「仅产物」物品（云芝竹沥羹只作为产物出现）
    expect(ids[0]).toBe(catalog.wuxingshi[0].cong);
    expect(collectRefreshWupinIds(catalog)).toEqual(ids);
    expect(ids.indexOf("5_89346")).toBeGreaterThan(ids.indexOf("5_89970"));
  });

  it("不再有 200 截断：超过 200 个材料也全部返回", () => {
    const materialIds = Array.from({ length: 320 }, (_, index) => `m_${String(index).padStart(3, "0")}`);
    const catalog = {
      ...mockCatalog(),
      wuxingshi: [],
      peifang: [
        {
          id: "big_recipe",
          mingcheng: "大配方",
          jiyi: "pengren",
          dengji: 1,
          jingli: 1,
          zhizuofei: 0,
          laiyuan: null,
          cailiao: materialIds.map((wupin_id) => ({ wupin_id, shuliang: 1 })),
          chuchan: [{ wupin_id: "out_1", min: 1, max: 1, quanzhong: 1_048_576 }],
        },
      ],
    } as unknown as Catalog;
    const ids = collectRefreshWupinIds(catalog);
    expect(ids.length).toBe(321);
    expect(ids.slice(0, 320)).toEqual(materialIds);
    expect(ids[320]).toBe("out_1");
  });

  it("目录为空时回退全部物品", () => {
    const catalog = { ...mockCatalog(), wuxingshi: [], peifang: [] };
    expect(collectRefreshWupinIds(catalog)).toEqual(catalog.wupin.map((wupin) => wupin.id));
  });
});
