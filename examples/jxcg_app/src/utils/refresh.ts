// ============================================================================
// 行情刷新范围：配方材料（去重、按使用频率排序）+ 五行石转换的两种石 + 产物，上限 200 个
//
// 依据 lead 联调约定：refresh_prices 一次别超过 ~200 个物品；
// 五行石数量少且精炼页依赖它们定价，放最前保证不被截断；材料决定成本，产物决定售价。
// ============================================================================

import type { Catalog } from "../types";

export const REFRESH_LIMIT = 200;

/** 收集一次行情刷新应覆盖的物品 id（材料优先，产物补足，去重，最多 200 个）。 */
export function collectRefreshWupinIds(catalog: Catalog, limit = REFRESH_LIMIT): string[] {
  // 五行石转换的两种石（数量很少，优先保证精炼页有价）
  const stones = new Set<string>();
  for (const zhuanhuan of catalog.wuxingshi) {
    stones.add(zhuanhuan.cong);
    stones.add(zhuanhuan.dao);
  }
  const usage = new Map<string, number>();
  const outputs = new Set<string>();
  for (const peifang of catalog.peifang) {
    for (const cailiao of peifang.cailiao) {
      usage.set(cailiao.wupin_id, (usage.get(cailiao.wupin_id) ?? 0) + Math.max(1, cailiao.shuliang));
    }
    for (const chuchan of peifang.chuchan) outputs.add(chuchan.wupin_id);
  }
  const ranked = [...usage.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([id]) => id);
  const ids = [...new Set([...stones, ...ranked, ...outputs])];
  // 目录里既没有配方也没有五行石时退回全部物品（首次启动/数据未导出）。
  if (ids.length === 0) return catalog.wupin.slice(0, limit).map((wupin) => wupin.id);
  return ids.length > limit ? ids.slice(0, limit) : ids;
}
