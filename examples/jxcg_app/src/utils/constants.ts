// ============================================================================
// 常量：本地存储键、枚举显示名、外部链接
// ============================================================================

import type { Jiyi, Pinlei, PaihangZhibiao, Quzheng, Settings, Shouxufei } from "../types";

/** 本地存储键（统一 jxcg_ 前缀，避免与 jpcg 存档串）。 */
export const STORAGE_KEYS = {
  theme: "jxcg_theme",
  settings: "jxcg_settings",
  zhijiOverride: "jxcg_zhiji_override",
  prices: "jxcg_prices",
  favorites: "jxcg_favorites",
  snapshots: "jxcg_snapshots",
  calcPrefs: "jxcg_calc_prefs",
  calcOverrides: "jxcg_calc_price_overrides",
  marketPrefs: "jxcg_market_prefs",
  rankPrefs: "jxcg_rank_prefs",
  wuxingshiPrefs: "jxcg_wuxingshi_prefs",
  wuxingshiOverrides: "jxcg_wuxingshi_overrides",
} as const;

/** 生活技艺显示名（顺序与 jxcg_api::Jiyi::ALL 一致）。 */
export const JIYI_LIST: ReadonlyArray<{ id: Jiyi; label: string }> = [
  { id: "pengren", label: "烹饪" },
  { id: "zhiyao", label: "制药" },
  { id: "fengren", label: "缝纫" },
  { id: "zhuzhao", label: "铸造" },
  { id: "zijiang", label: "梓匠" },
];

export const JIYI_LABEL: Record<Jiyi, string> = {
  pengren: "烹饪",
  zhiyao: "制药",
  fengren: "缝纫",
  zhuzhao: "铸造",
  zijiang: "梓匠",
};

/** 物品类别显示名。 */
export const PINLEI_LIST: ReadonlyArray<{ id: Pinlei; label: string }> = [
  { id: "cailiao", label: "材料" },
  { id: "chengpin", label: "成品" },
  { id: "xianhao" as Pinlei, label: "消耗品" },
  { id: "wuxingshi", label: "五行石" },
];

export const PINLEI_LABEL: Record<Pinlei, string> = {
  cailiao: "材料",
  chengpin: "成品",
  xiaohao: "消耗品",
  wuxingshi: "五行石",
};

/** 手续费取整方式显示名。 */
export const QUZHENG_LIST: ReadonlyArray<{ id: Quzheng; label: string }> = [
  { id: "half_up", label: "四舍五入" },
  { id: "floor", label: "向下取整" },
  { id: "ceil", label: "向上取整" },
];

export const QUZHENG_LABEL: Record<Quzheng, string> = {
  half_up: "四舍五入",
  floor: "向下取整",
  ceil: "向上取整",
};

/** 排行指标显示名。 */
export const PAIHANG_LIST: ReadonlyArray<{ id: PaihangZhibiao; label: string; hint: string }> = [
  { id: "lirun", label: "净利润/次", hint: "单次制作的净利润（铜）" },
  { id: "mei_jingli", label: "净利润/精力", hint: "每点精力净利润（铜）" },
  { id: "lirunlv", label: "利润率", hint: "净利润 / 总成本" },
];

export const PAIHANG_LABEL: Record<PaihangZhibiao, string> = {
  lirun: "净利润/次",
  mei_jingli: "净利润/精力",
  lirunlv: "利润率",
};

/** 默认设置（与 jxcg_api::Settings::default 一致）。 */
export const DEFAULT_SETTINGS: Settings = {
  fuwuqi_id: "jiandanqinxin",
  zhiji_id: "live-130",
  jingli_danjia: 0,
  jiyi_dengji: 0,
  auto_refresh: true,
  zhongjian_zizhi: true,
};

/** 默认值集（与 jxcg_api::Shouxufei::default 一致：5% HALF_UP + 保管费 80 银）。 */
export const DEFAULT_SHOUXUFEI: Shouxufei = {
  shuilv_bp: 500,
  quzheng: "half_up",
  baoguanfei: 8_000,
};

/** 行情 JSON / 存档 JSON 的版本号（结构变更时递增）。 */
export const JIAGE_FILE_VERSION = 1;
export const CUNDANG_FILE_VERSION = 1;

/** 本地快照最多保留条数（超出丢弃最旧）。 */
export const MAX_SNAPSHOTS = 40;

export const APP_NAME = "JXCG";
export const APP_TITLE = "JXCG · 剑网3 交易行计算器";
export const APP_VERSION = "0.1.0";
export const GITHUB_ISSUES_URL = "https://github.com/qinthirteen/jxcg/issues";

/** Tauri 行情刷新进度事件名（payload = RefreshProgress）。 */
export const REFRESH_PROGRESS_EVENT = "refresh-progress";

/** 界面提示：浏览器预览（mock）时的口径说明。 */
export const MOCK_HINT = "浏览器预览 · mock 数据";
