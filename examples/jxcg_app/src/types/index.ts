// ============================================================================
// JXCG 前端类型 —— 镜像 crates/jxcg_api/src/lib.rs 的 DTO 契约（单一来源）
//
// 约定（与 Rust 侧一致）：
// - 金额一律是「铜」为单位的 number；1 金 = 100 银 = 10_000 铜，1 砖 = 10_000 金；
// - 费率/成功率是万分比（shuilv_bp：500 = 5%）；利润率 lirunlv_bp 也是万分比；
// - 产出概率权重分母固定 QUANZHONG_FENMU = 2^20 = 1_048_576；
// - 字段名沿用 serde 的 snake_case，不要在这里改成 camelCase。
// ============================================================================

/** 金额：铜。1 金 = 100 银 = 10_000 铜；1 砖 = 10_000 金 = 100_000_000 铜。 */
export type Tong = number;

/** 产出概率权重分母（游戏数据口径，2^20）。 */
export const QUANZHONG_FENMU = 1_048_576;

// ---------------------------------------------------------------------------
// 枚举（serde rename_all = "snake_case"）
// ---------------------------------------------------------------------------

/** 生活技艺类型。 */
export type Jiyi = "pengren" | "zhiyao" | "fengren" | "zhuzhao" | "zijiang";

/** 物品类别。 */
export type Pinlei = "cailiao" | "chengpin" | "xiaohao" | "wuxingshi";

/** 手续费取整方式。 */
export type Quzheng = "half_up" | "floor" | "ceil";

/** 排行指标。 */
export type PaihangZhibiao = "lirun" | "mei_jingli" | "lirunlv";

// ---------------------------------------------------------------------------
// 静态数据（随包 TOML）
// ---------------------------------------------------------------------------

/** 数据版本。 */
export interface Banben {
  /** 等级（当前正式服 50，苍生铸世资料片等级回退） */
  level: number;
  /** 赛季序号 */
  season: number;
  /** 数据修订日期 YYYYMMDD */
  modified: number;
  /** 备注 */
  note: string;
}

/** 物品。 */
export interface Wupin {
  /** 游戏物品 id，形如 `5_30824` */
  id: string;
  mingcheng: string;
  pinlei: Pinlei;
  /** 一组堆叠数量（按组购买用） */
  dui?: number | null;
  /** 商店价（铜）；无则缺省 */
  shangdianjia?: Tong | null;
  /** 数据来源标识 */
  laiyuan?: string | null;
}

/** 配方材料需求。 */
export interface Cailiao {
  wupin_id: string;
  shuliang: number;
}

/** 配方产物槽（8 槽之一）。 */
export interface Chuchan {
  wupin_id: string;
  min: number;
  max: number;
  /** 概率权重，分母 QUANZHONG_FENMU */
  quanzhong: number;
}

/** 生活技艺配方。 */
export interface Peifang {
  /** 形如 `cooking_437` */
  id: string;
  mingcheng: string;
  jiyi: Jiyi;
  /** 配方等级要求（游戏内 RequireLevel） */
  dengji: number;
  /** 精力消耗（CostVigor） */
  jingli: number;
  /** 固定金币消耗（铜） */
  zhizuofei: Tong;
  cailiao: Cailiao[];
  chuchan: Chuchan[];
  laiyuan?: string | null;
}

/** 五行石转换（精炼）。 */
export interface Zhuanhuan {
  id: string;
  mingcheng: string;
  /** 低阶石物品 id */
  cong: string;
  /** 高阶石物品 id */
  dao: string;
  /** 每次投入低阶石数量 */
  xuyao: number;
  /** 精力消耗 */
  jingli: number;
  /** 金币消耗（铜） */
  jinbi: Tong;
  /** 成功率，万分比（4878 = 48.78%） */
  chenggonglv_bp: number;
  laiyuan?: string | null;
}

/** 五行石合成方案（游戏内「一键合成/自动合成」使用的配方集合）。 */
export interface WuxingshiFangan {
  id: string;
  mingcheng: string;
  /** 说明（面板口径/来源等） */
  note: string;
}

/** 单步合成结果槽：万分比概率与命中时产出数量。 */
export interface WuxingshiJieguo {
  wupin_id: string;
  /** 万分比：9276 = 92.76%；未列出的余量视为失败（材料损失、无产物） */
  bp: number;
  /** 命中时的产出数量（默认 1） */
  shuliang?: number;
}

/** 单步合成配方（主产物 + 材料 + 精力/加工费 + 概率结果）。 */
export interface WuxingshiHecheng {
  id: string;
  /** 所属合成方案 id */
  fangan: string;
  /** 主产物物品 id */
  chanpin: string;
  /** 精力消耗（点/次） */
  jingli: number;
  /** 加工费（铜/次） */
  jiagongfei: Tong;
  cailiao: Cailiao[];
  jieguo: WuxingshiJieguo[];
}

/** 服务器。 */
export interface Fuwuqi {
  id: string;
  mingcheng: string;
  /** 大区，如「电信区」「双线区」「无界区」 */
  daju?: string | null;
}

/** 手续费值集。 */
export interface Shouxufei {
  /** 万分比，500 = 5% */
  shuilv_bp: number;
  quzheng: Quzheng;
  /** 保管费（铜 / 24h） */
  baoguanfei: Tong;
}

/** 值集（费率/取整/保管费快照）。 */
export interface Zhiji {
  id: string;
  mingcheng: string;
  note: string;
  shouxufei: Shouxufei;
}

/** 随包完整目录（load_catalog 的返回）。 */
export interface Catalog {
  banben: Banben;
  wupin: Wupin[];
  peifang: Peifang[];
  wuxingshi: Zhuanhuan[];
  /** 五行石合成方案（data/wuxingshi.toml 的 `[[fangan]]`） */
  wuxingshi_fangan: WuxingshiFangan[];
  /** 五行石单步合成配方（data/wuxingshi.toml 的 `[[hecheng]]`） */
  wuxingshi_hecheng: WuxingshiHecheng[];
  fuwuqi: Fuwuqi[];
  zhiji: Zhiji[];
}

// ---------------------------------------------------------------------------
// 行情
// ---------------------------------------------------------------------------

/** 交易行挂单。 */
export interface Guadan {
  /** 上架时间戳（秒） */
  shijian: number;
  /** 数量 */
  shuliang: number;
  /** 单价（铜/件） */
  danjia: Tong;
}

/** 单个物品在某服的价格记录（load_prices / refresh_prices 的条目）。 */
export interface JiageJilu {
  wupin_id: string;
  fuwuqi_id: string;
  guadan: Guadan[];
  /** 最后更新时间戳（秒） */
  gengxin_shijian: number;
  /** jx3box / shoudong */
  laiyuan: string;
  /** 是否用户手动覆盖（优先于自动价） */
  shoudong: boolean;
}

/** 行情快照（本地历史，用于趋势图）。 */
export interface Kuaizhao {
  /** 快照时间戳（秒） */
  shijian: number;
  fuwuqi_id: string;
  /** 物品 id -> 当时最低单价（铜） */
  jiage: Record<string, Tong>;
}

// ---------------------------------------------------------------------------
// 计算请求 / 结果
// ---------------------------------------------------------------------------

/** 交易行净收入请求。 */
export interface NetRequest {
  /** 售价（铜/件） */
  shoujia: Tong;
  /** 挂单数量 */
  shuliang: number;
  shouxufei: Shouxufei;
  /** 挂单次数（保管费按次计） */
  guadan_cishu: number;
}

/** 交易行净收入结果。 */
export interface NetResult {
  /** 成交总额（铜） */
  zonge: Tong;
  /** 手续费（铜） */
  feiyong: Tong;
  /** 保管费合计（铜） */
  baoguanfei: Tong;
  /** 到手（铜） */
  daoshou: Tong;
}

/** 保本价请求。 */
export interface BaobenRequest {
  /** 总成本（铜） */
  chengben: Tong;
  shouxufei: Shouxufei;
  guadan_cishu: number;
}

/** 单配方成本利润请求。 */
export interface JisuanRequest {
  peifang_id: string;
  /** 制作次数 */
  cishu: number;
  /** 每次制作的挂单次数（保管费按次计） */
  guadan_cishu: number;
  /** 每点精力折合金额（铜） */
  jingli_danjia: Tong;
  shouxufei: Shouxufei;
  /**
   * 物品 id -> 单价（铜）：当前服务器全量行情 + 手填价（递归采购取价用）。
   * 缺失表示缺价。
   */
  jiage: Record<string, Tong>;
  /** 用户手动覆盖的物品 id（手填价；这些物品不再按中间产物展开） */
  shoudong: string[];
  /** 技艺等级（0-50，暂只影响展示加成，默认 0） */
  jiyi_dengji: number;
  /** 中间产物按自制成本展开摊入（默认 true；关闭则按行情价） */
  zhongjian_zizhi: boolean;
}

/** 材料明细行。 */
export interface CailiaoMingxi {
  wupin_id: string;
  mingcheng: string;
  shuliang: number;
  danjia: Tong;
  xiaojia: Tong;
  /** hangqing / shangdian / shoudong / quejia / zizhi */
  laiyuan: string;
  /** 是否为中间产物（自制展开行，单位成本含材料+精力+制作费） */
  zhongjian: boolean;
}

/** 产物明细行。 */
export interface ChuchanMingxi {
  wupin_id: string;
  mingcheng: string;
  /** 期望件数 */
  qiwang: number;
  danjia: Tong;
}

/** 单配方成本利润结果。 */
export interface JisuanResult {
  peifang_id: string;
  peifang_mingcheng: string;
  /** 材料成本合计（铜） */
  chengben_cailiao: Tong;
  /** 精力成本（铜） */
  chengben_jingli: Tong;
  /** 制作费（铜） */
  zhizuofei: Tong;
  /** 保管费合计（铜） */
  baoguanfei: Tong;
  /** 总成本（铜） */
  zong_chengben: Tong;
  /**
   * 单件成本（铜/件）= zong_chengben / 期望产出总件数（HALF_UP）。
   * 期望产出为 0 时等于 zong_chengben；多产出配方用它区分「每批总成本」与「单件成本」。
   */
  danjia_chengben: Tong;
  /** 期望产出价值（扣手续费前，铜） */
  shouru: Tong;
  /** 扣手续费后收入（铜） */
  jingshouru: Tong;
  /** 净利润（铜） */
  lirun: Tong;
  /** 利润率（万分比） */
  lirunlv_bp: number;
  /** 每精力净利润（铜，可能为负） */
  mei_jingli: Tong;
  /** 保本单价（铜/件，按期望产出折算） */
  baobenjia: Tong;
  /** 精力消耗合计 */
  jingli: number;
  /** 期望产出件数 */
  chuchan_qiwang: number;
  cailiao: CailiaoMingxi[];
  chuchan: ChuchanMingxi[];
  /** 缺价物品 id 列表（非空表示结果不完整） */
  quejia: string[];
  /** 顶层自制行成本合计（铜；已含在 chengben_cailiao 内，展示用、不重复加总） */
  chengben_zhongjian: Tong;
  /** 中间产物摊入的精力点数（最终配方自身精力之外的额外精力） */
  jingli_zhongjian: number;
  /** 中间产物展开明细（与材料表中的 zhongjian 行对应） */
  zhongjian: ZhongjianMingxi[];
  /**
   * 「不可获得」材料 id（quejia 的子集）：无行情/手填价、无 NPC 商店价，
   * 且没有任何配方产出它（无法靠自制展开），需人工补价或清理数据。
   */
  bukede: string[];
}

/** 中间产物自制明细（材料表中 zhongjian=true 行的展开信息）。 */
export interface ZhongjianMingxi {
  wupin_id: string;
  mingcheng: string;
  /** 产出该中间产物的配方 id */
  peifang_id: string;
  /** 本次制作需要的数量（含制作次数） */
  shuliang: number;
  /** 单位自制成本（铜/件） */
  danjia: Tong;
  /** 摊入小计（铜） */
  xiaojia: Tong;
  /** 摊入的精力点数 */
  jingli: number;
  /** 展开后的采购原料（可能仍含中间产物） */
  cailiao: CailiaoMingxi[];
}

/** 排行请求。 */
export interface PaihangRequest {
  /** 只排某技艺；null/缺省 = 全部 */
  jiyi?: Jiyi | null;
  jingli_danjia: Tong;
  shouxufei: Shouxufei;
  /** 物品 id -> 单价（铜）：当前服务器全量行情 + 手填价 */
  jiage: Record<string, Tong>;
  /** 用户手动覆盖的物品 id（手填价） */
  shoudong: string[];
  zhibiao: PaihangZhibiao;
  jiyi_dengji: number;
  /** 最多返回条数；null/缺省 = 全部 */
  limit?: number | null;
  /** 是否包含缺价配方（缺价行排在末尾，quejia=true）；默认 false */
  baohan_quejia?: boolean;
  /** 中间产物按自制成本展开（默认 true） */
  zhongjian_zizhi: boolean;
  /** 排行是否包含「只产中间产物」的配方（默认 false） */
  baohan_zhongjian: boolean;
}

/** 排行行。 */
export interface PaihangRow {
  peifang_id: string;
  mingcheng: string;
  jiyi: Jiyi;
  lirun: Tong;
  lirunlv_bp: number;
  mei_jingli: Tong;
  jingli: number;
  /** 是否缺价（缺价的默认不参与排行，但结果里保留供提示） */
  quejia: boolean;
  /** 该配方是否只产中间产物（默认不参与排行，baohan_zhongjian 可显示） */
  zhongjian: boolean;
}

/** 五行石转换请求。 */
export interface ZhuanhuanRequest {
  zhuanhuan_id: string;
  cishu: number;
  jingli_danjia: Tong;
  shouxufei: Shouxufei;
  jiage: Record<string, Tong>;
}

/** 五行石转换结果。 */
export interface ZhuanhuanResult {
  zhuanhuan_id: string;
  mingcheng: string;
  /** 期望投入次数（1 / 成功率） */
  qiwang_cishu: number;
  /** 期望总成本（铜） */
  zong_chengben: Tong;
  /** 期望产出价值（扣手续费前） */
  shouru: Tong;
  /** 净利润（铜） */
  lirun: Tong;
  lirunlv_bp: number;
  /** 缺价物品 id */
  quejia: string[];
}

// ---------------------------------------------------------------------------
// 五行石一键合成（面板配方链）
// ---------------------------------------------------------------------------

/** 五行石一键合成请求。 */
export interface WuxingshiLianRequest {
  fangan_id: string;
  /** 目标物品 id（六级 = `5_24428`，默认） */
  mubiao_wupin_id: string;
  /** 目标件数 */
  shuliang: number;
  /** 每点精力折合金额（铜） */
  jingli_danjia: Tong;
  shouxufei: Shouxufei;
  /** 物品 id -> 单价（铜）：行情/手填 > 商店价 > 缺价 */
  jiage: Record<string, Tong>;
  /** 覆盖「合 6 意外出 8 级」概率（万分比）；null = 用数据里的 bp（0 = 忽略） */
  baoji_bp?: number | null;
}

/** 五行石合成副产物（期望量）。 */
export interface WuxingshiFuchanpin {
  wupin_id: string;
  mingcheng: string;
  /** 期望产出件数 */
  qiwang: number;
  danjia: Tong;
  /** 折价收入（铜，HALF_UP）；缺价时为 0 */
  shouru: Tong;
}

/** 五行石合成步骤（按单步配方聚合的期望值）。 */
export interface WuxingshiBuzhou {
  hecheng_id: string;
  chanpin: string;
  mingcheng: string;
  /** 期望合成次数（含成功率放大） */
  cishu: number;
  /** 该步骤精力合计（期望，HALF_UP 到整数） */
  jingli: number;
  /** 该步骤加工费合计（期望，HALF_UP 到铜） */
  jiagongfei: Tong;
}

/** 五行石一键合成结果。 */
export interface WuxingshiLianResult {
  fangan_id: string;
  fangan_mingcheng: string;
  mubiao_wupin_id: string;
  mubiao_mingcheng: string;
  shuliang: number;
  /** 采购原料明细（一级石等无配方原料；数量为期望值 HALF_UP 到整数，展示口径） */
  cailiao: CailiaoMingxi[];
  /** 材料成本（铜，期望值一次性 HALF_UP） */
  cailiao_chengben: Tong;
  /** 精力合计（点，期望值 HALF_UP 到整数） */
  jingli: number;
  /** 精力折金（铜） */
  jingli_chengben: Tong;
  /** 加工费合计（铜） */
  jiagongfei: Tong;
  /** 副产物期望收益明细 */
  fuchanpin: WuxingshiFuchanpin[];
  /** 副产物折价收入合计（铜，冲抵成本） */
  fuchanpin_shouru: Tong;
  /** 净成本（铜）= 材料 + 精力折金 + 加工费 − 副产物收入 */
  zong_chengben: Tong;
  /** 单位净成本（铜/颗，= zong_chengben / shuliang 四舍五入） */
  danjia: Tong;
  /** 保本单价（铜/颗，按手续费反推的最小挂单价） */
  baobenjia: Tong;
  /** 目标物市场价（铜/颗；缺价为 0 并进 quejia） */
  shichangjia: Tong;
  /** 净利润（铜）= 市场价 × 数量 − 手续费 − 净成本 */
  lirun: Tong;
  /** 利润率（万分比） */
  lirunlv_bp: number;
  /** 合成步骤明细（按数据文件顺序，仅含实际用到的配方） */
  buzhou: WuxingshiBuzhou[];
  /** 缺价物品 id（排序去重） */
  quejia: string[];
}

// ---------------------------------------------------------------------------
// 设置 / 刷新进度
// ---------------------------------------------------------------------------

/** 用户设置（本地存档）。 */
export interface Settings {
  fuwuqi_id: string;
  zhiji_id: string;
  /** 每点精力折合金额（铜） */
  jingli_danjia: Tong;
  /** 技艺等级（0-50） */
  jiyi_dengji: number;
  /** 是否联网自动拉价 */
  auto_refresh: boolean;
  /** 中间产物按自制成本摊入（默认 true） */
  zhongjian_zizhi: boolean;
}

/** 行情刷新进度事件负载（事件名 refresh_progress）。 */
export interface RefreshProgress {
  /** 已完成数量 */
  done: number;
  /** 总数量 */
  total: number;
  /** 失败数量 */
  failed: number;
}

/** 行情刷新结果。 */
export interface RefreshResult {
  fuwuqi_id: string;
  gengxin_shijian: number;
  chenggong: number;
  shibai: number;
  jiage: JiageJilu[];
}

// ---------------------------------------------------------------------------
// 前端本地状态（UI 层，非 DTO 契约）
// ---------------------------------------------------------------------------

/** 本地缓存的单物品行情（由 JiageJilu/JX3BOX 最低挂单价归并而来）。 */
export interface JiageEntry {
  /** 最低单价（铜/件） */
  danjia: Tong;
  /** 更新时间戳（秒） */
  gengxin_shijian: number;
  /** jx3box / shoudong / mock */
  laiyuan: string;
  /** 用户手动覆盖（优先于自动价，刷新不覆盖） */
  shoudong: boolean;
}

/** 行情缓存：key 为 `${fuwuqi_id}|${wupin_id}`（见 utils/jiage.ts）。 */
export type JiageBiao = Record<string, JiageEntry>;

/** 行情 JSON 导出文件（行情页「导出 JSON」）。 */
export interface JiageDaochuFile {
  version: number;
  fuwuqi_id: string;
  daochu_shijian: number;
  jiage: Array<{
    wupin_id: string;
    danjia: Tong;
    gengxin_shijian: number;
    laiyuan: string;
    shoudong: boolean;
  }>;
}

/** 存档（设置页「存档导入导出」）：设置 + 行情 + 收藏 + 快照 + 值集覆盖。 */
export interface CundangFile {
  version: number;
  daochu_shijian: number;
  settings: Settings;
  zhiji_override: Shouxufei | null;
  jiage: JiageBiao;
  favorites: string[];
  snapshots: Kuaizhao[];
}

/** 计算页偏好（本地持久化）。 */
export type CalcMode = "recipe" | "manual";

export interface CalcPrefs {
  mode: CalcMode;
  peifang_id: string;
  search: string;
  jiyi: Jiyi | "all";
  only_fav: boolean;
  cishu: number;
  guadan_cishu: number;
  /** 手动模式：售价（铜/件） */
  shoujia: Tong;
  /** 手动模式：挂单数量 */
  shuliang: number;
  /** 手动模式：保本计算用的总成本（铜） */
  chengben: Tong;
}

/** 行情页偏好（本地持久化）。 */
export interface MarketPrefs {
  search: string;
  pinlei: Pinlei | "all";
  only_fav: boolean;
  selected_wupin_id: string;
}

/** 五行石页偏好（本地持久化）。 */
export interface WuxingshiPrefs {
  /** 选中的转换配方 id（catalog.wuxingshi[].id） */
  zhuanhuan_id: string;
  /** 目标成功次数（cishu；期望投入 = cishu / 成功率） */
  cishu: number;
}

/** 五行石「一键合成」页偏好（本地持久化）。 */
export interface WuxingshiLianPrefs {
  /** 选中的合成方案 id（catalog.wuxingshi_fangan[].id） */
  fangan_id: string;
  /** 目标物品 id（默认六级 `5_24428`） */
  mubiao_wupin_id: string;
  /** 目标数量 */
  shuliang: number;
  /** 合 6 出 8 级概率覆盖（万分比，0 = 忽略） */
  baoji_bp: number;
}

/** 排行页偏好（本地持久化）。 */
export interface RankPrefs {
  zhibiao: PaihangZhibiao;
  jiyi: Jiyi | "all";
  limit: number;
  baohan_quejia: boolean;
  /** 是否包含「只产中间产物」的配方（默认 false） */
  baohan_zhongjian: boolean;
}
