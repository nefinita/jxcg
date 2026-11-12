// ============================================================================
// 本地持久化 schema：版本号 + 运行时校验（localStorage 数据可能被手改/过期）
//
// 校验失败的旧数据不会让页面崩溃，统一回退到 fallback 默认值；
// 未来版本（version 大于当前）不尝试降级解释，同样回退。
// ============================================================================

import type {
  CalcPrefs,
  JiageBiao,
  JiageEntry,
  Kuaizhao,
  MarketPrefs,
  RankPrefs,
  Settings,
  Shouxufei,
  Tong,
  WuxingshiLianPrefs,
  WuxingshiPrefs,
} from "../types";
import { DEFAULT_SETTINGS } from "../utils/constants";

/** 持久化信封：{ version, data }。 */
export interface PersistentSchema<T> {
  version: number;
  validate: (value: unknown) => value is T;
  /** 旧版本升级（可选）；返回未知时交给 validate 再校验。 */
  migrate?: (value: unknown, version: number) => unknown;
}

// ---------------------------------------------------------------------------
// 基础类型守卫
// ---------------------------------------------------------------------------

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

export function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim() !== "";
}

// ---------------------------------------------------------------------------
// 设置 / 值集
// ---------------------------------------------------------------------------

export function isSettings(value: unknown): value is Settings {
  if (!isRecord(value)) return false;
  return (
    typeof value.fuwuqi_id === "string" &&
    typeof value.zhiji_id === "string" &&
    isFiniteNumber(value.jingli_danjia) &&
    isFiniteNumber(value.jiyi_dengji) &&
    typeof value.auto_refresh === "boolean"
  );
}

export function isShouxufei(value: unknown): value is Shouxufei {
  if (!isRecord(value)) return false;
  return (
    isFiniteNumber(value.shuilv_bp) &&
    (value.quzheng === "half_up" || value.quzheng === "floor" || value.quzheng === "ceil") &&
    isFiniteNumber(value.baoguanfei)
  );
}

/** 旧版设置缺字段时补默认值。 */
function migrateSettings(value: unknown): unknown {
  if (!isRecord(value)) return value;
  return { ...DEFAULT_SETTINGS, ...value };
}

export const settingsSchema: PersistentSchema<Settings> = {
  version: 1,
  validate: isSettings,
  migrate: migrateSettings,
};

export const zhijiOverrideSchema: PersistentSchema<Shouxufei | null> = {
  version: 1,
  validate: (value: unknown): value is Shouxufei | null => value === null || isShouxufei(value),
};

// ---------------------------------------------------------------------------
// 行情缓存 / 收藏 / 快照
// ---------------------------------------------------------------------------

export function isJiageEntry(value: unknown): value is JiageEntry {
  if (!isRecord(value)) return false;
  return (
    isFiniteNumber(value.danjia) &&
    isFiniteNumber(value.gengxin_shijian) &&
    typeof value.laiyuan === "string" &&
    typeof value.shoudong === "boolean"
  );
}

export function isJiageBiao(value: unknown): value is JiageBiao {
  if (!isRecord(value)) return false;
  return Object.values(value).every(isJiageEntry);
}

export function isKuaizhao(value: unknown): value is Kuaizhao {
  if (!isRecord(value)) return false;
  if (!isFiniteNumber(value.shijian) || typeof value.fuwuqi_id !== "string") return false;
  if (!isRecord(value.jiage)) return false;
  return Object.values(value.jiage).every(isFiniteNumber);
}

export function isPriceOverrides(value: unknown): value is Record<string, Tong> {
  if (!isRecord(value)) return false;
  return Object.values(value).every(isFiniteNumber);
}

export const pricesSchema: PersistentSchema<JiageBiao> = { version: 1, validate: isJiageBiao };
export const favoritesSchema: PersistentSchema<string[]> = {
  version: 1,
  validate: (value: unknown): value is string[] =>
    Array.isArray(value) && value.every((v) => typeof v === "string"),
};
export const snapshotsSchema: PersistentSchema<Kuaizhao[]> = {
  version: 1,
  validate: (value: unknown): value is Kuaizhao[] =>
    Array.isArray(value) && value.every(isKuaizhao),
};
export const calcOverridesSchema: PersistentSchema<Record<string, Tong>> = {
  version: 1,
  validate: isPriceOverrides,
};

// ---------------------------------------------------------------------------
// 页面偏好
// ---------------------------------------------------------------------------

export const DEFAULT_CALC_PREFS: CalcPrefs = {
  mode: "recipe",
  peifang_id: "",
  search: "",
  jiyi: "all",
  only_fav: false,
  cishu: 1,
  guadan_cishu: 1,
  shoujia: 12_000_000,
  shuliang: 1,
  chengben: 10_000_000,
};

export const DEFAULT_MARKET_PREFS: MarketPrefs = {
  search: "",
  pinlei: "all",
  only_fav: false,
  selected_wupin_id: "",
};

export const DEFAULT_RANK_PREFS: RankPrefs = {
  zhibiao: "lirun",
  jiyi: "all",
  limit: 10,
  baohan_quejia: false,
  baohan_zhongjian: false,
};

function isJiyiValue(value: unknown): boolean {
  return (
    value === "pengren" ||
    value === "zhiyao" ||
    value === "fengren" ||
    value === "zhuzhao" ||
    value === "zijiang"
  );
}

export function isCalcPrefs(value: unknown): value is CalcPrefs {
  if (!isRecord(value)) return false;
  return (
    (value.mode === "recipe" || value.mode === "manual") &&
    typeof value.peifang_id === "string" &&
    typeof value.search === "string" &&
    (value.jiyi === "all" || isJiyiValue(value.jiyi)) &&
    typeof value.only_fav === "boolean" &&
    isFiniteNumber(value.cishu) &&
    isFiniteNumber(value.guadan_cishu) &&
    isFiniteNumber(value.shoujia) &&
    isFiniteNumber(value.shuliang) &&
    isFiniteNumber(value.chengben)
  );
}

export function isMarketPrefs(value: unknown): value is MarketPrefs {
  if (!isRecord(value)) return false;
  return (
    typeof value.search === "string" &&
    (value.pinlei === "all" ||
      value.pinlei === "cailiao" ||
      value.pinlei === "chengpin" ||
      value.pinlei === "xiaohao" ||
      value.pinlei === "wuxingshi") &&
    typeof value.only_fav === "boolean" &&
    typeof value.selected_wupin_id === "string"
  );
}

export function isRankPrefs(value: unknown): value is RankPrefs {
  if (!isRecord(value)) return false;
  return (
    (value.zhibiao === "lirun" || value.zhibiao === "mei_jingli" || value.zhibiao === "lirunlv") &&
    (value.jiyi === "all" || isJiyiValue(value.jiyi)) &&
    isFiniteNumber(value.limit) &&
    typeof value.baohan_quejia === "boolean" &&
    typeof value.baohan_zhongjian === "boolean"
  );
}

export const calcPrefsSchema: PersistentSchema<CalcPrefs> = { version: 1, validate: isCalcPrefs };
export const marketPrefsSchema: PersistentSchema<MarketPrefs> = { version: 1, validate: isMarketPrefs };
export const rankPrefsSchema: PersistentSchema<RankPrefs> = {
  version: 2,
  validate: isRankPrefs,
  // v1 没有「包含中间产物」开关：升级时补默认 false。
  migrate: (value, version) =>
    version < 2 && isRecord(value) ? { ...value, baohan_zhongjian: false } : value,
};

// ---------------------------------------------------------------------------
// 五行石页偏好
// ---------------------------------------------------------------------------

export const DEFAULT_WUXINGSHI_PREFS: WuxingshiPrefs = {
  zhuanhuan_id: "",
  cishu: 1,
};

export function isWuxingshiPrefs(value: unknown): value is WuxingshiPrefs {
  if (!isRecord(value)) return false;
  return typeof value.zhuanhuan_id === "string" && isFiniteNumber(value.cishu);
}

export const wuxingshiPrefsSchema: PersistentSchema<WuxingshiPrefs> = {
  version: 1,
  validate: isWuxingshiPrefs,
};

// ---------------------------------------------------------------------------
// 五行石一键合成页偏好
// ---------------------------------------------------------------------------

export const DEFAULT_WUXINGSHI_LIAN_PREFS: WuxingshiLianPrefs = {
  fangan_id: "",
  mubiao_wupin_id: "5_24428",
  shuliang: 1,
  baoji_bp: 0,
};

export function isWuxingshiLianPrefs(value: unknown): value is WuxingshiLianPrefs {
  if (!isRecord(value)) return false;
  return (
    typeof value.fangan_id === "string" &&
    typeof value.mubiao_wupin_id === "string" &&
    isFiniteNumber(value.shuliang) &&
    isFiniteNumber(value.baoji_bp)
  );
}

export const wuxingshiLianPrefsSchema: PersistentSchema<WuxingshiLianPrefs> = {
  version: 1,
  validate: isWuxingshiLianPrefs,
};
