// ============================================================================
// 行情缓存工具：JiageJilu → JiageEntry、按服务器取价、快照生成/合并
//
// 前端把后端返回的行情记录归并成「最低挂单价」缓存（JiageBiao），
// 手动覆盖（shoudong=true）优先于自动价，刷新时不被覆盖。
// ============================================================================

import type { JiageBiao, JiageEntry, JiageJilu, Kuaizhao, Tong, Wupin } from "../types";
import { nowSeconds } from "./format";
import { MAX_SNAPSHOTS } from "./constants";

/** 缓存 key：服务器 + 物品。 */
export function jiageKey(fuwuqiId: string, wupinId: string): string {
  return `${fuwuqiId}|${wupinId}`;
}

/** 取某服某物品的缓存行情。 */
export function getJiage(biao: JiageBiao, fuwuqiId: string, wupinId: string): JiageEntry | undefined {
  return biao[jiageKey(fuwuqiId, wupinId)];
}

/** 写入一条缓存行情（返回新对象，便于 setState）。 */
export function setJiage(
  biao: JiageBiao,
  fuwuqiId: string,
  wupinId: string,
  entry: JiageEntry,
): JiageBiao {
  return { ...biao, [jiageKey(fuwuqiId, wupinId)]: entry };
}

/** 从挂单列表取最低单价（忽略非正数挂单）；无挂单返回 0。 */
export function zuidiDanjia(guadan: JiageJilu["guadan"]): Tong {
  let min = 0;
  for (const g of guadan) {
    if (!Number.isFinite(g.danjia) || g.danjia <= 0) continue;
    if (min === 0 || g.danjia < min) min = g.danjia;
  }
  return min;
}

/** 单条 JiageJilu → JiageEntry（无有效挂单返回 null）。 */
export function jiageEntryOf(jilu: JiageJilu): JiageEntry | null {
  const danjia = zuidiDanjia(jilu.guadan);
  if (danjia <= 0) return null;
  return {
    danjia,
    gengxin_shijian: jilu.gengxin_shijian || nowSeconds(),
    laiyuan: jilu.laiyuan || "jx3box",
    shoudong: !!jilu.shoudong,
  };
}

/**
 * 把后端行情合并进本地缓存：
 * - 默认保留本地手动覆盖价（shoudong=true）不被自动刷新覆盖；
 * - 传入 force=true 时用后端数据整体覆盖（用于导入/手动覆盖之后的同步）。
 */
export function mergeJiageJilu(
  biao: JiageBiao,
  fuwuqiId: string,
  jiluList: JiageJilu[],
  options: { keepShoudong?: boolean } = {},
): JiageBiao {
  const keepShoudong = options.keepShoudong ?? true;
  const next: JiageBiao = { ...biao };
  for (const jilu of jiluList) {
    if (jilu.fuwuqi_id && jilu.fuwuqi_id !== fuwuqiId) continue;
    const entry = jiageEntryOf(jilu);
    if (!entry) continue;
    const key = jiageKey(fuwuqiId, jilu.wupin_id);
    const old = next[key];
    if (keepShoudong && old?.shoudong && !entry.shoudong) continue;
    next[key] = entry;
  }
  return next;
}

/** 取某服若干物品的「有价」映射（用于 calculate_* / rank_* 请求的 jiage）。 */
export function jiageMapOf(
  biao: JiageBiao,
  fuwuqiId: string,
  wupinIds: Iterable<string>,
): Record<string, Tong> {
  const result: Record<string, Tong> = {};
  for (const id of wupinIds) {
    const entry = getJiage(biao, fuwuqiId, id);
    if (entry && entry.danjia > 0) result[id] = entry.danjia;
  }
  return result;
}

/**
 * 展示/统计用单价（与后端 task-12 口径一致）：手填 > NPC 商店价 > 行情 > 缺价。
 *
 * - NPC 有售（shangdianjia > 0）就按商店价，不做「商店/行情取低」；
 * - 手填价（缓存 shoudong=true）是用户显式覆盖，优先级最高；
 * - 都没有返回 null（缺价）。
 */
export function danjiaOf(
  biao: JiageBiao,
  fuwuqiId: string,
  wupin: Wupin,
): { danjia: Tong; laiyuan: string } | null {
  const entry = getJiage(biao, fuwuqiId, wupin.id);
  // 1) 手填覆盖
  if (entry && entry.shoudong && entry.danjia > 0) {
    return { danjia: entry.danjia, laiyuan: "shoudong" };
  }
  // 2) NPC 商店价（> 0 才视为有售）
  if (wupin.shangdianjia && wupin.shangdianjia > 0) {
    return { danjia: wupin.shangdianjia, laiyuan: "shangdian" };
  }
  // 3) 行情
  if (entry && entry.danjia > 0) {
    return { danjia: entry.danjia, laiyuan: entry.laiyuan };
  }
  return null;
}

/** 当前服务器全量行情映射（用于计算/排行的采购取价，含递归中间产物）。 */
export function quanliangJiageMap(biao: JiageBiao, fuwuqiId: string): Record<string, Tong> {
  const prefix = `${fuwuqiId}|`;
  const map: Record<string, Tong> = {};
  for (const [key, entry] of Object.entries(biao)) {
    if (!key.startsWith(prefix)) continue;
    if (entry.danjia > 0) map[key.slice(prefix.length)] = entry.danjia;
  }
  return map;
}

/** 当前服务器手动覆盖价的物品 id（排序稳定；手填中间产物时不再展开）。 */
export function shoudongIdsOf(biao: JiageBiao, fuwuqiId: string): string[] {
  const prefix = `${fuwuqiId}|`;
  const ids: string[] = [];
  for (const [key, entry] of Object.entries(biao)) {
    if (!key.startsWith(prefix)) continue;
    if (entry.shoudong) ids.push(key.slice(prefix.length));
  }
  return ids.sort();
}

/** 用当前缓存生成一条行情快照。 */
export function snapshotOf(biao: JiageBiao, fuwuqiId: string, wupinIds: Iterable<string>): Kuaizhao {
  const jiage: Record<string, Tong> = {};
  for (const id of wupinIds) {
    const entry = getJiage(biao, fuwuqiId, id);
    if (entry && entry.danjia > 0) jiage[id] = entry.danjia;
  }
  return { shijian: nowSeconds(), fuwuqi_id: fuwuqiId, jiage };
}

/** 追加快照（保留最近 MAX_SNAPSHOTS 条，新的在后）。 */
export function appendSnapshot(snapshots: Kuaizhao[], snapshot: Kuaizhao): Kuaizhao[] {
  const next = [...snapshots, snapshot];
  return next.length > MAX_SNAPSHOTS ? next.slice(next.length - MAX_SNAPSHOTS) : next;
}

/** 某服某物品的历史价格点（按时间升序）。 */
export function trendOf(
  snapshots: Kuaizhao[],
  fuwuqiId: string,
  wupinId: string,
): Array<{ shijian: number; danjia: Tong }> {
  return snapshots
    .filter((s) => s.fuwuqi_id === fuwuqiId && s.jiage[wupinId] > 0)
    .map((s) => ({ shijian: s.shijian, danjia: s.jiage[wupinId] }))
    .sort((a, b) => a.shijian - b.shijian);
}
