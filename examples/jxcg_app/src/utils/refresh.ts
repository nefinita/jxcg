// ============================================================================
// 行情刷新范围：五行石转换的两种石 + 全部配方材料 + 全部配方产物
//
// - 去重、稳定排序（石头 → 材料 → 产物，均按 catalog 顺序），**不再截断**；
// - 目录完全为空时才回退 catalog.wupin；
// - 全量刷新由用户显式触发（首次约 1–2 分钟），失败保留最近一次价格。
// ============================================================================

import type { Catalog } from "../types";

/** 收集一次行情刷新应覆盖的物品 id（全量相关物品，去重、稳定排序、无上限）。 */
export function collectRefreshWupinIds(catalog: Catalog): string[] {
  const ids: string[] = [];
  const seen = new Set<string>();
  const push = (id: string) => {
    if (!id || seen.has(id)) return;
    seen.add(id);
    ids.push(id);
  };

  // 1) 五行石转换的两种石（数量少，精炼页依赖它们定价）
  for (const zhuanhuan of catalog.wuxingshi) {
    push(zhuanhuan.cong);
    push(zhuanhuan.dao);
  }
  // 2) 全部配方材料（成本侧）
  for (const peifang of catalog.peifang) {
    for (const cailiao of peifang.cailiao) push(cailiao.wupin_id);
  }
  // 3) 全部配方产物（售价侧）
  for (const peifang of catalog.peifang) {
    for (const chuchan of peifang.chuchan) push(chuchan.wupin_id);
  }

  // 目录里既没有配方也没有五行石时退回全部物品（首次启动/数据未导出）。
  if (ids.length === 0) {
    for (const wupin of catalog.wupin) push(wupin.id);
  }
  return ids;
}
