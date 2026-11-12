// ============================================================================
// 中间产物（中间产物自制成本）派生工具
//
// 定义：同时是「某配方的产物」与「某配方的材料」的物品（数据里约 32 个）。
// - 计算时按自制成本递归展开（自身材料 + 精力×精力单价 + 制作费，按期望产出折算）；
// - 只产中间产物的配方默认不进入成品排行（baohan_zhongjian 可显示）。
// ============================================================================

import type { Catalog, Peifang } from "../types";

/** 派生中间产物 id 集合（产物 ∩ 材料）。 */
export function deriveZhongjianIds(catalog: Catalog): Set<string> {
  const materials = new Set<string>();
  for (const peifang of catalog.peifang) {
    for (const cailiao of peifang.cailiao) materials.add(cailiao.wupin_id);
  }
  const ids = new Set<string>();
  for (const peifang of catalog.peifang) {
    for (const chuchan of peifang.chuchan) {
      if (materials.has(chuchan.wupin_id)) ids.add(chuchan.wupin_id);
    }
  }
  return ids;
}

/** 配方是否为「只产中间产物」的配方（产物非空且全部是中间产物）。 */
export function isZhongjianPeifang(peifang: Peifang, zhongjianIds: ReadonlySet<string>): boolean {
  return peifang.chuchan.length > 0 && peifang.chuchan.every((chuchan) => zhongjianIds.has(chuchan.wupin_id));
}

/** 单次制作的某物品期望产出件数（同物品多槽按权重累加）。 */
export function chuchanQiwangOf(chuchan: Peifang["chuchan"], wupinId: string): number {
  const QUANZHONG_FENMU = 1_048_576;
  return chuchan
    .filter((slot) => slot.wupin_id === wupinId)
    .reduce(
      (sum, slot) => sum + (slot.quanzhong / QUANZHONG_FENMU) * ((slot.min + slot.max) / 2),
      0,
    );
}
