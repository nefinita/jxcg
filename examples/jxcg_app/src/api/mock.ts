// ============================================================================
// 浏览器 mock 后端：window.__TAURI__ 不存在时由 api/commands.ts 调用。
//
// - 计算口径镜像 crates/jxcg_core（fee/craft/rank/convert），便于浏览器预览验证；
// - 行情数据由 mockData.ts 的示例数据 deterministic 生成，刷新时做小幅抖动；
// - 刷新进度通过 refresh_progress 事件回放（与 Tauri 侧事件同名同 payload）。
//
// 注意：mock 只用于开发预览，真实结果以桌面端 Rust 后端为准。
// ============================================================================

import type {
  BaobenRequest,
  Catalog,
  CailiaoMingxi,
  Chuchan,
  ChuchanMingxi,
  JiageJilu,
  JisuanRequest,
  JisuanResult,
  Kuaizhao,
  NetRequest,
  NetResult,
  PaihangRequest,
  PaihangRow,
  Peifang,
  RefreshProgress,
  RefreshResult,
  Settings,
  Shouxufei,
  Tong,
  WuxingshiHecheng,
  WuxingshiLianRequest,
  WuxingshiLianResult,
  ZhongjianMingxi,
  ZhuanhuanRequest,
  ZhuanhuanResult,
} from "../types";
import { QUANZHONG_FENMU } from "../types";
import { DEFAULT_SETTINGS, REFRESH_PROGRESS_EVENT } from "../utils/constants";
import { nowSeconds } from "../utils/format";
import { MOCK_BASE_JIAGE, MOCK_CATALOG, MOCK_SERVER_FACTOR } from "./mockData";
import { chuchanQiwangOf, deriveZhongjianIds, isZhongjianPeifang } from "../utils/zhongjian";

// ---------------------------------------------------------------------------
// 事件总线（refresh_progress）
// ---------------------------------------------------------------------------

type MockEventListener = (event: { payload: unknown }) => void;

const listeners = new Map<string, Set<MockEventListener>>();

function emit(event: string, payload: unknown) {
  const set = listeners.get(event);
  if (!set) return;
  for (const cb of [...set]) {
    try {
      cb({ payload });
    } catch {
      /* mock 监听方异常不影响刷新流程 */
    }
  }
}

/** 模拟 Tauri event.listen：返回取消监听函数。 */
export async function mockListen(
  event: string,
  cb: (e: { payload: unknown }) => void,
): Promise<() => void> {
  const set = listeners.get(event) ?? new Set<MockEventListener>();
  set.add(cb);
  listeners.set(event, set);
  return () => {
    set.delete(cb);
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ---------------------------------------------------------------------------
// 行情状态（localStorage 模拟后端行情文件；刷新后浏览器重载仍保留）
// ---------------------------------------------------------------------------

const MOCK_PRICE_KEY = "jxcg_mock_prices";

interface MockPriceState {
  [fuwuqiId: string]: {
    gengxin_shijian: number;
    jiage: Record<string, Tong>;
    /** 手动覆盖（刷新时保留，与后端 upsert_price 语义一致） */
    shoudong?: Record<string, boolean>;
  };
}

function readMockPriceState(): MockPriceState {
  try {
    const raw = localStorage.getItem(MOCK_PRICE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return {};
    return parsed as MockPriceState;
  } catch {
    return {};
  }
}

function writeMockPriceState(state: MockPriceState) {
  try {
    localStorage.setItem(MOCK_PRICE_KEY, JSON.stringify(state));
  } catch {
    /* 存储不可用时仅内存生效 */
  }
}

function serverFactor(fuwuqiId: string): number {
  return MOCK_SERVER_FACTOR[fuwuqiId] ?? 1;
}

/** 当前 mock 行情单价（刷新状态 > 基础价 × 服系数）；null 表示缺价。 */
function mockDanjia(fuwuqiId: string, wupinId: string): Tong | null {
  const state = readMockPriceState();
  const cached = state[fuwuqiId]?.jiage[wupinId];
  if (cached && cached > 0) return cached;
  const base = MOCK_BASE_JIAGE[wupinId];
  if (!base) return null;
  return Math.max(1, Math.round(base * serverFactor(fuwuqiId)));
}

function priceJilu(fuwuqiId: string, wupinId: string, danjia: Tong, shoudong = false): JiageJilu {
  const t = nowSeconds();
  return {
    wupin_id: wupinId,
    fuwuqi_id: fuwuqiId,
    guadan: [
      { shijian: t - 3600, shuliang: 5, danjia },
      { shijian: t - 600, shuliang: 3, danjia: Math.round(danjia * 1.05) },
    ],
    gengxin_shijian: t - 300,
    laiyuan: shoudong ? "shoudong" : "jx3box",
    shoudong,
  };
}

// ---------------------------------------------------------------------------
// 费用计算（镜像 jxcg_core::fee）
// ---------------------------------------------------------------------------

function jisuanShouxufei(amount: Tong, cfg: Shouxufei): Tong {
  if (amount <= 0 || cfg.shuilv_bp <= 0) return 0;
  const raw = amount * cfg.shuilv_bp;
  let fee: number;
  if (cfg.quzheng === "floor") fee = Math.floor(raw / 10_000);
  else if (cfg.quzheng === "ceil") fee = Math.ceil(raw / 10_000);
  else fee = Math.floor((raw + 5_000) / 10_000);
  fee = Math.max(0, fee);
  return Math.min(fee, amount);
}

function mockCalculateNet(req: NetRequest): NetResult {
  if (req.shoujia < 0) throw new Error("售价不能为负");
  if (req.shouxufei.shuilv_bp < 0 || req.shouxufei.shuilv_bp >= 10_000) {
    throw new Error("手续费率必须在 0%~100% 之间");
  }
  const zonge = req.shoujia * Math.max(1, req.shuliang);
  const feiyong = jisuanShouxufei(zonge, req.shouxufei);
  const baoguanfei = req.shouxufei.baoguanfei * Math.max(1, req.guadan_cishu);
  const daoshou = zonge - feiyong - baoguanfei;
  return { zonge, feiyong, baoguanfei, daoshou };
}

function mockCalculateBaoben(req: BaobenRequest): Tong {
  if (req.chengben < 0) throw new Error("成本不能为负");
  if (req.shouxufei.shuilv_bp < 0 || req.shouxufei.shuilv_bp >= 10_000) {
    throw new Error("手续费率必须在 0%~100% 之间");
  }
  const target = req.chengben + req.shouxufei.baoguanfei * Math.max(1, req.guadan_cishu);
  if (target === 0) return 0;
  let hi = 1;
  while (hi - jisuanShouxufei(hi, req.shouxufei) < target) hi *= 2;
  let lo = 0;
  while (lo < hi) {
    const mid = lo + Math.floor((hi - lo) / 2);
    if (mid - jisuanShouxufei(mid, req.shouxufei) >= target) hi = mid;
    else lo = mid + 1;
  }
  return lo;
}

// ---------------------------------------------------------------------------
// 配方计算（镜像 jxcg_core::craft / rank / convert）
// ---------------------------------------------------------------------------

function findWupin(catalog: Catalog, id: string) {
  return catalog.wupin.find((w) => w.id === id);
}

/** 单次制作的期望产出件数。 */
function chuchanQiwang(chuchan: Chuchan[]): number {
  return chuchan.reduce(
    (sum, slot) =>
      sum + (slot.quanzhong / QUANZHONG_FENMU) * ((slot.min + slot.max) / 2),
    0,
  );
}

/** 空手填集合（convert/wuxingshi 等请求没有 shoudong 字段时使用）。 */
const EMPTY_SHOUDONG: ReadonlySet<string> = new Set<string>();

/**
 * 材料/产物单价（与后端 jxcg_core::price 同口径）：手填 > NPC 商店价 > 行情 > 缺价。
 * 手填仅在「id 在手填集合且 jiage 有值」时生效；商店价不做与行情的取低。
 */
function chuchanDanjia(
  catalog: Catalog,
  jiage: Record<string, Tong>,
  wupinId: string,
  quejia: string[],
  shoudong: ReadonlySet<string> = EMPTY_SHOUDONG,
): { danjia: Tong; laiyuan: string } {
  const jiageValue = jiage[wupinId];
  // 1) 手填覆盖（必须同时有手填价，否则回落下一级）
  if (shoudong.has(wupinId) && typeof jiageValue === "number" && jiageValue > 0) {
    return { danjia: jiageValue, laiyuan: "shoudong" };
  }
  // 2) NPC 商店价（> 0 才视为有售）
  const wupin = findWupin(catalog, wupinId);
  if (wupin?.shangdianjia && wupin.shangdianjia > 0) {
    return { danjia: wupin.shangdianjia, laiyuan: "shangdian" };
  }
  // 3) 行情
  if (typeof jiageValue === "number" && jiageValue > 0) {
    return { danjia: jiageValue, laiyuan: "hangqing" };
  }
  // 4) 缺价
  if (!quejia.includes(wupinId)) quejia.push(wupinId);
  return { danjia: 0, laiyuan: "quejia" };
}

// ---------------------------------------------------------------------------
// 中间产物自制成本展开（近似后端 task-5 口径）
// ---------------------------------------------------------------------------

/** 中间产物集合（产物 ∩ 材料），随包目录不变，模块级派生一次。 */
const ZHONGJIAN_IDS: ReadonlySet<string> = deriveZhongjianIds(MOCK_CATALOG);

/** 中间产物 -> 产出配方（同物品多配方取 (dengji, id) 最小者，对齐 jxcg_core::zhongjian）。 */
const ZHONGJIAN_PRODUCERS: ReadonlyMap<string, Peifang> = (() => {
  const map = new Map<string, Peifang>();
  for (const peifang of MOCK_CATALOG.peifang) {
    for (const chuchan of peifang.chuchan) {
      if (!ZHONGJIAN_IDS.has(chuchan.wupin_id)) continue;
      const current = map.get(chuchan.wupin_id);
      if (
        !current ||
        peifang.dengji < current.dengji ||
        (peifang.dengji === current.dengji && peifang.id.localeCompare(current.id) < 0)
      ) {
        map.set(chuchan.wupin_id, peifang);
      }
    }
  }
  return map;
})();

interface ZizhiCailiao {
  wupin_id: string;
  shuliang: number;
  danjia: Tong;
  laiyuan: string;
  zhongjian: boolean;
}

interface ZizhiCost {
  /** 单位自制成本（铜/件） */
  danjia: Tong;
  /** 每件摊入的精力点数（含嵌套中间产物） */
  jingli_per_unit: number;
  peifang_id: string;
  cailiao: ZizhiCailiao[];
}

interface ZizhiContext {
  jiage: Record<string, Tong>;
  jingli_danjia: Tong;
  quejia: string[];
  /** 手填价物品：不按中间产物展开，直接用 jiage 里的手填价采购 */
  shoudong: ReadonlySet<string>;
  cache: Map<string, ZizhiCost | null>;
  visiting: Set<string>;
}

/** 递归计算某中间产物的单位自制成本（材料 + 精力×单价 + 制作费，按期望产出折算）。 */
function zizhiUnitCost(wupinId: string, ctx: ZizhiContext): ZizhiCost | null {
  if (ctx.cache.has(wupinId)) return ctx.cache.get(wupinId) ?? null;
  // 环引用（A→B→A）时降级为行情价，避免死循环。
  if (ctx.visiting.has(wupinId)) return null;
  const producer = ZHONGJIAN_PRODUCERS.get(wupinId);
  if (!producer) return null;
  const qiwang = chuchanQiwangOf(producer.chuchan, wupinId);
  if (qiwang <= 0) return null;

  ctx.visiting.add(wupinId);
  let cailiaoCost = 0;
  let nestedJingli = 0;
  const cailiao: ZizhiCailiao[] = [];
  for (const item of producer.cailiao) {
    let danjia: Tong;
    let laiyuan: string;
    let zhongjian = false;
    if (ctx.shoudong.has(item.wupin_id)) {
      // 手填价优先：不再递归展开，直接用 jiage 里的手填价（缺值则回落商店/行情/缺价）。
      const manual = chuchanDanjia(MOCK_CATALOG, ctx.jiage, item.wupin_id, ctx.quejia, ctx.shoudong);
      danjia = manual.danjia;
      laiyuan = manual.laiyuan;
    } else if (ZHONGJIAN_IDS.has(item.wupin_id)) {
      const nested = zizhiUnitCost(item.wupin_id, ctx);
      if (nested) {
        danjia = nested.danjia;
        laiyuan = "zizhi";
        zhongjian = true;
        nestedJingli += nested.jingli_per_unit * item.shuliang;
      } else {
        const fallback = chuchanDanjia(MOCK_CATALOG, ctx.jiage, item.wupin_id, ctx.quejia, ctx.shoudong);
        danjia = fallback.danjia;
        laiyuan = fallback.laiyuan;
      }
    } else {
      const found = chuchanDanjia(MOCK_CATALOG, ctx.jiage, item.wupin_id, ctx.quejia, ctx.shoudong);
      danjia = found.danjia;
      laiyuan = found.laiyuan;
    }
    cailiaoCost += danjia * item.shuliang;
    cailiao.push({ wupin_id: item.wupin_id, shuliang: item.shuliang, danjia, laiyuan, zhongjian });
  }
  const jingliPerCraft = producer.jingli + nestedJingli;
  const perCraftCost = cailiaoCost + producer.jingli * ctx.jingli_danjia + producer.zhizuofei;
  ctx.visiting.delete(wupinId);
  const result: ZizhiCost = {
    danjia: Math.round(perCraftCost / qiwang),
    jingli_per_unit: jingliPerCraft / qiwang,
    peifang_id: producer.id,
    cailiao,
  };
  ctx.cache.set(wupinId, result);
  return result;
}

function wupinMingcheng(wupinId: string): string {
  return findWupin(MOCK_CATALOG, wupinId)?.mingcheng ?? wupinId;
}

/** 从缺价列表筛「不可获得」材料：是配方材料、但没有任何配方产出它（与 craft.rs 同口径）。 */
function bukedeWupin(quejia: readonly string[]): string[] {
  const isCailiao = (id: string) =>
    MOCK_CATALOG.peifang.some((peifang) => peifang.cailiao.some((item) => item.wupin_id === id));
  const isChanchu = (id: string) =>
    MOCK_CATALOG.peifang.some((peifang) => peifang.chuchan.some((slot) => slot.wupin_id === id));
  return quejia.filter((id) => isCailiao(id) && !isChanchu(id));
}

function mockCalculateRecipe(req: JisuanRequest): JisuanResult {
  const peifang = MOCK_CATALOG.peifang.find((p) => p.id === req.peifang_id);
  if (!peifang) throw new Error(`配方不存在：${req.peifang_id}`);
  const cishu = Math.max(1, Math.trunc(req.cishu));
  const zhongjianZizhi = req.zhongjian_zizhi !== false;
  // zhongjian_zizhi=false 时严格旧行为：shoudong 不参与取价（对齐 craft.rs）。
  const shoudong: ReadonlySet<string> = zhongjianZizhi ? new Set(req.shoudong ?? []) : EMPTY_SHOUDONG;
  const quejia: string[] = [];
  const ctx: ZizhiContext = {
    jiage: req.jiage,
    jingli_danjia: req.jingli_danjia,
    quejia,
    shoudong,
    cache: new Map(),
    visiting: new Set(),
  };

  const cailiao: CailiaoMingxi[] = [];
  const zhongjian: ZhongjianMingxi[] = [];
  let chengbenCailiao: Tong = 0;
  let chengbenZhongjian: Tong = 0;
  let jingliZhongjian = 0;

  for (const item of peifang.cailiao) {
    const need = item.shuliang * cishu;
    const zizhi =
      zhongjianZizhi && ZHONGJIAN_IDS.has(item.wupin_id) && !shoudong.has(item.wupin_id)
        ? zizhiUnitCost(item.wupin_id, ctx)
        : null;
    if (zizhi) {
      const xiaojia = zizhi.danjia * need;
      chengbenCailiao += xiaojia;
      chengbenZhongjian += xiaojia;
      const jingliPoints = zizhi.jingli_per_unit * need;
      jingliZhongjian += jingliPoints;
      const producer = ZHONGJIAN_PRODUCERS.get(item.wupin_id);
      const runs = producer ? need / Math.max(chuchanQiwangOf(producer.chuchan, item.wupin_id), 1e-9) : 1;
      cailiao.push({
        wupin_id: item.wupin_id,
        mingcheng: wupinMingcheng(item.wupin_id),
        shuliang: item.shuliang,
        danjia: zizhi.danjia,
        xiaojia,
        laiyuan: "zizhi",
        zhongjian: true,
      });
      zhongjian.push({
        wupin_id: item.wupin_id,
        mingcheng: wupinMingcheng(item.wupin_id),
        peifang_id: zizhi.peifang_id,
        shuliang: need,
        danjia: zizhi.danjia,
        xiaojia,
        jingli: Math.round(jingliPoints),
        cailiao: zizhi.cailiao.map((row) => {
          const qty = Math.max(1, Math.round(row.shuliang * runs));
          return {
            wupin_id: row.wupin_id,
            mingcheng: wupinMingcheng(row.wupin_id),
            shuliang: qty,
            danjia: row.danjia,
            xiaojia: row.danjia * qty,
            laiyuan: row.laiyuan,
            zhongjian: row.zhongjian,
          };
        }),
      });
      continue;
    }
    const { danjia, laiyuan } = chuchanDanjia(MOCK_CATALOG, req.jiage, item.wupin_id, quejia, shoudong);
    const xiaojia = danjia * need;
    chengbenCailiao += xiaojia;
    cailiao.push({
      wupin_id: item.wupin_id,
      mingcheng: wupinMingcheng(item.wupin_id),
      shuliang: item.shuliang,
      danjia,
      xiaojia,
      laiyuan,
      zhongjian: false,
    });
  }

  const jingli = peifang.jingli * cishu;
  const chengbenJingli = jingli * req.jingli_danjia;
  const zhizuofei = peifang.zhizuofei * cishu;
  const baoguanfei = req.shouxufei.baoguanfei * Math.max(1, req.guadan_cishu) * cishu;
  const zongChengben = chengbenCailiao + chengbenJingli + zhizuofei + baoguanfei;

  const qiwangYici = chuchanQiwang(peifang.chuchan);
  const chuchanQiwangTotal = qiwangYici * cishu;
  const chuchan: ChuchanMingxi[] = [];
  let shouru: Tong = 0;
  for (const slot of peifang.chuchan) {
    const p = slot.quanzhong / QUANZHONG_FENMU;
    const qiwang = p * ((slot.min + slot.max) / 2) * cishu;
    const { danjia } = chuchanDanjia(MOCK_CATALOG, req.jiage, slot.wupin_id, quejia, shoudong);
    shouru += Math.round(qiwang * danjia);
    chuchan.push({
      wupin_id: slot.wupin_id,
      mingcheng: wupinMingcheng(slot.wupin_id),
      qiwang,
      danjia,
    });
  }
  if (req.shouxufei.shuilv_bp < 0 || req.shouxufei.shuilv_bp >= 10_000) {
    throw new Error("手续费率必须在 0%~100% 之间");
  }
  const feiyong = jisuanShouxufei(shouru, req.shouxufei);
  const jingshouru = shouru - feiyong;
  const lirun = jingshouru - zongChengben;
  const lirunlvBp = zongChengben > 0 ? Math.round((lirun / zongChengben) * 10_000) : 0;
  // 单件成本 = 总成本 / 期望产出（HALF_UP；无产出时退化为总成本）。
  const danjiaChengben = chuchanQiwangTotal > 0 ? Math.round(zongChengben / chuchanQiwangTotal) : zongChengben;
  const jingliTotal = jingli + Math.round(jingliZhongjian);
  const meiJingli = jingliTotal > 0 ? Math.trunc(lirun / jingliTotal) : lirun;
  const quejiaSorted = [...new Set(quejia)].sort();
  const baobenTotal = mockBaobenValue(zongChengben, req.shouxufei);
  const baobenjia =
    chuchanQiwangTotal > 0 ? Math.ceil(baobenTotal / chuchanQiwangTotal) : baobenTotal;

  return {
    peifang_id: peifang.id,
    peifang_mingcheng: peifang.mingcheng,
    chengben_cailiao: chengbenCailiao,
    chengben_jingli: chengbenJingli,
    zhizuofei,
    baoguanfei,
    zong_chengben: zongChengben,
    danjia_chengben: danjiaChengben,
    shouru,
    jingshouru,
    lirun,
    lirunlv_bp: lirunlvBp,
    mei_jingli: meiJingli,
    baobenjia,
    jingli,
    chuchan_qiwang: chuchanQiwangTotal,
    cailiao,
    chuchan,
    quejia: quejiaSorted,
    bukede: bukedeWupin(quejiaSorted),
    chengben_zhongjian: chengbenZhongjian,
    jingli_zhongjian: Math.round(jingliZhongjian),
    zhongjian,
  };
}

/** 保本总售价（成本已含保管费，不再重复计挂单次数，对齐 craft.rs）。 */
function mockBaobenValue(chengben: Tong, cfg: Shouxufei): Tong {
  if (chengben <= 0) return 0;
  let hi = 1;
  while (hi - jisuanShouxufei(hi, cfg) < chengben) hi *= 2;
  let lo = 0;
  while (lo < hi) {
    const mid = lo + Math.floor((hi - lo) / 2);
    if (mid - jisuanShouxufei(mid, cfg) >= chengben) hi = mid;
    else lo = mid + 1;
  }
  return lo;
}

function mockRankRecipes(req: PaihangRequest): PaihangRow[] {
  const rows: PaihangRow[] = [];
  const zhongjianZizhi = req.zhongjian_zizhi !== false;
  const baohanZhongjian = req.baohan_zhongjian === true;
  for (const peifang of MOCK_CATALOG.peifang) {
    if (req.jiyi && peifang.jiyi !== req.jiyi) continue;
    // 只产中间产物的配方默认不进入成品排行。
    const zhongjianPeifang = isZhongjianPeifang(peifang, ZHONGJIAN_IDS);
    if (zhongjianPeifang && !baohanZhongjian) continue;
    const result = mockCalculateRecipe({
      peifang_id: peifang.id,
      cishu: 1,
      guadan_cishu: 1,
      jingli_danjia: req.jingli_danjia,
      shouxufei: req.shouxufei,
      jiage: req.jiage,
      jiyi_dengji: req.jiyi_dengji,
      zhongjian_zizhi: zhongjianZizhi,
      shoudong: req.shoudong ?? [],
    });
    const quejia = result.quejia.length > 0;
    if (quejia && !req.baohan_quejia) continue;
    rows.push({
      peifang_id: peifang.id,
      mingcheng: peifang.mingcheng,
      jiyi: peifang.jiyi,
      lirun: result.lirun,
      lirunlv_bp: result.lirunlv_bp,
      mei_jingli: result.mei_jingli,
      jingli: peifang.jingli,
      quejia,
      zhongjian: zhongjianPeifang,
    });
  }
  rows.sort((a, b) => {
    // 缺价行排在末尾（「包含缺价配方」开启时）。
    if (a.quejia !== b.quejia) return a.quejia ? 1 : -1;
    let primary = 0;
    if (req.zhibiao === "lirun") primary = b.lirun - a.lirun;
    else if (req.zhibiao === "mei_jingli") primary = b.mei_jingli - a.mei_jingli;
    else primary = b.lirunlv_bp - a.lirunlv_bp;
    return primary !== 0 ? primary : a.peifang_id.localeCompare(b.peifang_id);
  });
  return typeof req.limit === "number" && req.limit > 0 ? rows.slice(0, req.limit) : rows;
}

function mockCalculateConvert(req: ZhuanhuanRequest): ZhuanhuanResult {
  const zhuanhuan = MOCK_CATALOG.wuxingshi.find((z) => z.id === req.zhuanhuan_id);
  if (!zhuanhuan) throw new Error(`转换配方不存在：${req.zhuanhuan_id}`);
  const cishu = Math.max(1, Math.trunc(req.cishu));
  const quejia: string[] = [];
  const conga = chuchanDanjia(MOCK_CATALOG, req.jiage, zhuanhuan.cong, quejia).danjia;
  const daojia = chuchanDanjia(MOCK_CATALOG, req.jiage, zhuanhuan.dao, quejia).danjia;
  const chenggonglvBp = Math.min(10_000, Math.max(1, zhuanhuan.chenggonglv_bp));
  const p = chenggonglvBp / 10_000;
  const qiwangCishu = (1 / p) * cishu;
  const yiciChengben =
    conga * zhuanhuan.xuyao + zhuanhuan.jinbi + zhuanhuan.jingli * req.jingli_danjia;
  const zongChengben = Math.round(yiciChengben * (1 / p) * cishu);
  const shouru = Math.round(p * cishu * daojia);
  const feiyong = jisuanShouxufei(shouru, req.shouxufei);
  const jingshouru = shouru - feiyong;
  const lirun = jingshouru - zongChengben;
  const lirunlvBp = zongChengben > 0 ? Math.round((lirun / zongChengben) * 10_000) : 0;
  return {
    zhuanhuan_id: zhuanhuan.id,
    mingcheng: zhuanhuan.mingcheng,
    qiwang_cishu: qiwangCishu,
    zong_chengben: zongChengben,
    shouru,
    lirun,
    lirunlv_bp: lirunlvBp,
    quejia: [...new Set(quejia)].sort(),
  };
}

// ---------------------------------------------------------------------------
// 五行石一键合成（面板配方链，镜像 WuxingshiLianResult 契约）
// ---------------------------------------------------------------------------

interface LianContext {
  jiage: Record<string, Tong>;
  jingli_danjia: Tong;
  baoji_bp: number | null;
  byChanpin: Map<string, WuxingshiHecheng>;
  quejia: string[];
  /** 原料期望用量（无配方可展开的一级石等） */
  cailiao: Map<string, number>;
  /** 副产物期望用量（2/5/6/8 级） */
  fuchanpin: Map<string, number>;
  /** 每步期望合成次数 */
  buzhou: Map<string, { hecheng: WuxingshiHecheng; cishu: number }>;
  visiting: Set<string>;
}

/** 行情/商店/缺价取价（缺价按 0 计并记入 quejia）。 */
function lianPriceInfo(ctx: LianContext, wupinId: string): { danjia: Tong; laiyuan: string } {
  // wuxingshi 请求没有手填字段：商店价 > 行情 > 缺价（与 craft 同一 helper）
  return chuchanDanjia(MOCK_CATALOG, ctx.jiage, wupinId, ctx.quejia, EMPTY_SHOUDONG);
}

function lianPrice(ctx: LianContext, wupinId: string): Tong {
  return lianPriceInfo(ctx, wupinId).danjia;
}

function lianPriceLaiyuan(ctx: LianContext, wupinId: string): string {
  return lianPriceInfo(ctx, wupinId).laiyuan;
}

/** 按期望递归展开：主产物概率 <100% 时按 1/p 放大投入，副产物按概率累计。 */
function needLianItem(chanpinId: string, qty: number, ctx: LianContext) {
  if (qty <= 0) return;
  const hecheng = ctx.byChanpin.get(chanpinId);
  if (!hecheng) {
    // 无合成配方：一级石等直接作为采购原料
    ctx.cailiao.set(chanpinId, (ctx.cailiao.get(chanpinId) ?? 0) + qty);
    return;
  }
  if (ctx.visiting.has(chanpinId)) {
    // 数据成环时降级为采购，避免死循环
    ctx.cailiao.set(chanpinId, (ctx.cailiao.get(chanpinId) ?? 0) + qty);
    return;
  }
  const mainBp = hecheng.jieguo
    .filter((jieguo) => jieguo.wupin_id === chanpinId)
    .reduce((sum, jieguo) => sum + jieguo.bp * (jieguo.shuliang ?? 1), 0);
  if (mainBp <= 0) throw new Error(`合成配方「${hecheng.id}」缺少主产物概率`);
  const attempts = (qty * 10_000) / mainBp;
  const previous = ctx.buzhou.get(hecheng.id);
  ctx.buzhou.set(hecheng.id, { hecheng, cishu: (previous?.cishu ?? 0) + attempts });
  ctx.visiting.add(chanpinId);
  for (const item of hecheng.cailiao) {
    needLianItem(item.wupin_id, attempts * item.shuliang, ctx);
  }
  for (const jieguo of hecheng.jieguo) {
    if (jieguo.wupin_id === chanpinId) continue;
    let bp = jieguo.bp;
    // 合 6 出 8 级：数据里 bp=0 占位，由请求 baoji_bp 覆盖（None = 保持数据值）
    if (bp === 0 && ctx.baoji_bp !== null) bp = ctx.baoji_bp;
    if (bp <= 0) continue;
    const qiwang = attempts * (bp / 10_000) * (jieguo.shuliang ?? 1);
    ctx.fuchanpin.set(jieguo.wupin_id, (ctx.fuchanpin.get(jieguo.wupin_id) ?? 0) + qiwang);
  }
  ctx.visiting.delete(chanpinId);
}

function mockCalculateWuxingshi(req: WuxingshiLianRequest): WuxingshiLianResult {
  const fangan = MOCK_CATALOG.wuxingshi_fangan.find((item) => item.id === req.fangan_id);
  if (!fangan) throw new Error(`合成方案不存在：${req.fangan_id}`);
  const shuliang = Math.max(1, Math.trunc(req.shuliang));
  const mubiao = findWupin(MOCK_CATALOG, req.mubiao_wupin_id);
  const hechengs = MOCK_CATALOG.wuxingshi_hecheng.filter((item) => item.fangan === fangan.id);
  const byChanpin = new Map<string, WuxingshiHecheng>();
  for (const hecheng of hechengs) {
    if (!byChanpin.has(hecheng.chanpin)) byChanpin.set(hecheng.chanpin, hecheng);
  }
  if (!byChanpin.has(req.mubiao_wupin_id)) {
    throw new Error(
      `方案「${fangan.mingcheng}」没有「${mubiao?.mingcheng ?? req.mubiao_wupin_id}」的合成配方（数据待补）`,
    );
  }
  const baojiBp =
    typeof req.baoji_bp === "number" && Number.isFinite(req.baoji_bp)
      ? Math.max(0, Math.min(10_000, Math.trunc(req.baoji_bp)))
      : null;
  const ctx: LianContext = {
    jiage: req.jiage,
    jingli_danjia: req.jingli_danjia,
    baoji_bp: baojiBp,
    byChanpin,
    quejia: [],
    cailiao: new Map(),
    fuchanpin: new Map(),
    buzhou: new Map(),
    visiting: new Set(),
  };
  needLianItem(req.mubiao_wupin_id, shuliang, ctx);

  // 采购原料明细（数量为期望值 HALF_UP 到整数，展示口径）
  const cailiao: CailiaoMingxi[] = [];
  let cailiaoChengbenRaw = 0;
  for (const [wupinId, qiwang] of ctx.cailiao) {
    const danjia = lianPrice(ctx, wupinId);
    cailiaoChengbenRaw += qiwang * danjia;
    cailiao.push({
      wupin_id: wupinId,
      mingcheng: wupinMingcheng(wupinId),
      shuliang: Math.round(qiwang),
      danjia,
      xiaojia: Math.round(qiwang * danjia),
      laiyuan: lianPriceLaiyuan(ctx, wupinId),
      zhongjian: false,
    });
  }
  const cailiaoChengben = Math.round(cailiaoChengbenRaw);

  // 副产物期望收益（2/5/6/8 级）
  const fuchanpin = [...ctx.fuchanpin.entries()]
    .filter(([, qiwang]) => qiwang > 0)
    .map(([wupinId, qiwang]) => {
      const danjia = lianPrice(ctx, wupinId);
      return {
        wupin_id: wupinId,
        mingcheng: wupinMingcheng(wupinId),
        qiwang,
        danjia,
        shouru: Math.round(qiwang * danjia),
      };
    });
  const fuchanpinShouru = fuchanpin.reduce((sum, row) => sum + row.shouru, 0);

  // 分步明细（按数据文件顺序）
  const order = new Map(hechengs.map((hecheng, index) => [hecheng.id, index]));
  const buzhou = [...ctx.buzhou.entries()]
    .sort((a, b) => (order.get(a[0]) ?? 0) - (order.get(b[0]) ?? 0))
    .map(([hechengId, { hecheng, cishu }]) => ({
      hecheng_id: hechengId,
      chanpin: hecheng.chanpin,
      mingcheng: wupinMingcheng(hecheng.chanpin),
      cishu,
      jingli: Math.round(cishu * hecheng.jingli),
      jiagongfei: Math.round(cishu * hecheng.jiagongfei),
    }));
  const jingliRaw = [...ctx.buzhou.values()].reduce(
    (sum, { hecheng, cishu }) => sum + cishu * hecheng.jingli,
    0,
  );
  const jiagongfeiRaw = [...ctx.buzhou.values()].reduce(
    (sum, { hecheng, cishu }) => sum + cishu * hecheng.jiagongfei,
    0,
  );
  const jingli = Math.round(jingliRaw);
  const jingliChengben = Math.round(jingliRaw * req.jingli_danjia);
  const jiagongfei = Math.round(jiagongfeiRaw);
  const zongChengben = cailiaoChengben + jingliChengben + jiagongfei - fuchanpinShouru;
  const danjia = Math.round(zongChengben / shuliang);
  const shichangjia = lianPrice(ctx, req.mubiao_wupin_id);
  const zongshoujia = shichangjia * shuliang;
  const feiyong = jisuanShouxufei(zongshoujia, req.shouxufei);
  const lirun = zongshoujia - feiyong - zongChengben;
  const lirunlvBp = zongChengben > 0 ? Math.round((lirun / zongChengben) * 10_000) : 0;
  const baobenTotal = mockBaobenValue(zongChengben, req.shouxufei);
  const baobenjia = Math.ceil(baobenTotal / shuliang);

  return {
    fangan_id: fangan.id,
    fangan_mingcheng: fangan.mingcheng,
    mubiao_wupin_id: req.mubiao_wupin_id,
    mubiao_mingcheng: mubiao?.mingcheng ?? req.mubiao_wupin_id,
    shuliang,
    cailiao,
    cailiao_chengben: cailiaoChengben,
    jingli,
    jingli_chengben: jingliChengben,
    jiagongfei,
    fuchanpin,
    fuchanpin_shouru: fuchanpinShouru,
    zong_chengben: zongChengben,
    danjia,
    baobenjia,
    shichangjia,
    lirun,
    lirunlv_bp: lirunlvBp,
    buzhou,
    quejia: [...new Set(ctx.quejia)].sort(),
  };
}

// ---------------------------------------------------------------------------
// 设置
// ---------------------------------------------------------------------------

const MOCK_SETTINGS_KEY = "jxcg_mock_settings";

function mockLoadSettings(): Settings {
  try {
    const raw = localStorage.getItem(MOCK_SETTINGS_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return { ...DEFAULT_SETTINGS };
    return { ...DEFAULT_SETTINGS, ...(parsed as Partial<Settings>) };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

function mockSaveSettings(settings: Settings) {
  try {
    localStorage.setItem(MOCK_SETTINGS_KEY, JSON.stringify(settings));
  } catch {
    /* 忽略 */
  }
}

// ---------------------------------------------------------------------------
// 命令分发
// ---------------------------------------------------------------------------

function asRecord(args: unknown): Record<string, unknown> {
  return typeof args === "object" && args !== null ? (args as Record<string, unknown>) : {};
}

/** 浏览器模式下的「Tauri 命令」实现。 */
export async function mockInvoke(command: string, rawArgs?: Record<string, unknown>): Promise<unknown> {
  const args = asRecord(rawArgs);
  switch (command) {
    case "ping": {
      await sleep(20);
      return "pong（mock）";
    }
    case "load_catalog":
    case "reload_catalog": {
      await sleep(120);
      return structuredClone(MOCK_CATALOG) satisfies Catalog;
    }
    case "load_prices": {
      await sleep(100);
      const fuwuqiId = String(args.fuwuqiId ?? "");
      const shoudongMap = readMockPriceState()[fuwuqiId]?.shoudong ?? {};
      const jilu: JiageJilu[] = [];
      for (const wupin of MOCK_CATALOG.wupin) {
        const danjia = mockDanjia(fuwuqiId, wupin.id);
        if (danjia) jilu.push(priceJilu(fuwuqiId, wupin.id, danjia, !!shoudongMap[wupin.id]));
      }
      return jilu;
    }
    case "upsert_price": {
      await sleep(60);
      const fuwuqiId = String(args.fuwuqiId ?? "");
      const wupinId = String(args.wupinId ?? "");
      const danjia = Number(args.danjia ?? 0);
      if (!wupinId || !Number.isFinite(danjia) || danjia < 0) throw new Error("价格不合法");
      const state = readMockPriceState();
      state[fuwuqiId] = {
        gengxin_shijian: nowSeconds(),
        jiage: { ...(state[fuwuqiId]?.jiage ?? {}), [wupinId]: danjia },
        shoudong: { ...(state[fuwuqiId]?.shoudong ?? {}), [wupinId]: true },
      };
      writeMockPriceState(state);
      return null;
    }
    case "refresh_prices": {
      const fuwuqiId = String(args.fuwuqiId ?? "");
      const wupinIds = Array.isArray(args.wupinIds) ? (args.wupinIds as string[]) : [];
      const total = wupinIds.length;
      let done = 0;
      let failed = 0;
      const state = readMockPriceState();
      const cached: Record<string, Tong> = { ...(state[fuwuqiId]?.jiage ?? {}) };
      const shoudong: Record<string, boolean> = { ...(state[fuwuqiId]?.shoudong ?? {}) };
      const result: JiageJilu[] = [];
      let seed = (Date.now() % 2_147_483_647) || 1;
      const rnd = () => {
        seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648;
        return seed / 2_147_483_648;
      };
      const factor = serverFactor(fuwuqiId);
      for (const wupinId of wupinIds) {
        await sleep(12 + rnd() * 20);
        const base = MOCK_BASE_JIAGE[wupinId];
        if (!base) {
          failed += 1;
        } else if (shoudong[wupinId] && cached[wupinId]) {
          // 手动覆盖价优先：刷新保留原价（与后端 upsert_price 语义一致）。
          result.push(priceJilu(fuwuqiId, wupinId, cached[wupinId], true));
        } else {
          const danjia = Math.max(1, Math.round(base * factor * (0.92 + rnd() * 0.16)));
          cached[wupinId] = danjia;
          result.push(priceJilu(fuwuqiId, wupinId, danjia));
        }
        done += 1;
        emit(REFRESH_PROGRESS_EVENT, { done, total, failed } satisfies RefreshProgress);
      }
      state[fuwuqiId] = { gengxin_shijian: nowSeconds(), jiage: cached, shoudong };
      writeMockPriceState(state);
      return {
        fuwuqi_id: fuwuqiId,
        gengxin_shijian: nowSeconds(),
        chenggong: total - failed,
        shibai: failed,
        jiage: result,
      } satisfies RefreshResult;
    }
    case "load_settings": {
      await sleep(60);
      return mockLoadSettings();
    }
    case "save_settings": {
      await sleep(60);
      const settings = args.settings as Settings | undefined;
      if (!settings) throw new Error("设置不能为空");
      mockSaveSettings(settings);
      return null;
    }
    case "calculate_net": {
      await sleep(80);
      return mockCalculateNet(args.req as NetRequest);
    }
    case "calculate_baoben": {
      await sleep(80);
      return mockCalculateBaoben(args.req as BaobenRequest);
    }
    case "calculate_recipe": {
      await sleep(100);
      return mockCalculateRecipe(args.req as JisuanRequest);
    }
    case "rank_recipes": {
      await sleep(140);
      return mockRankRecipes(args.req as PaihangRequest);
    }
    case "calculate_convert": {
      await sleep(80);
      return mockCalculateConvert(args.req as ZhuanhuanRequest);
    }
    case "calculate_wuxingshi": {
      await sleep(120);
      return mockCalculateWuxingshi(args.req as WuxingshiLianRequest);
    }
    default:
      throw new Error(`mock 未实现的命令：${command}`);
  }
}

/** 供测试/调试：判断当前是否浏览器 mock 模式。 */
export function isMockMode(): boolean {
  return typeof window !== "undefined" && !window.__TAURI__?.core?.invoke;
}

/** 供测试复用：mock 行情快照（当前货币价格映射）。 */
export function mockPriceMap(fuwuqiId: string): Record<string, Tong> {
  const result: Record<string, Tong> = {};
  for (const wupin of MOCK_CATALOG.wupin) {
    const danjia = mockDanjia(fuwuqiId, wupin.id);
    if (danjia) result[wupin.id] = danjia;
  }
  return result;
}

/** 供测试复用：mock 目录。 */
export function mockCatalog(): Catalog {
  return structuredClone(MOCK_CATALOG);
}

/** 供测试复用：构造快照（趋势图 debug 用）。 */
export function mockSnapshot(fuwuqiId: string, shijian = nowSeconds()): Kuaizhao {
  return { shijian, fuwuqi_id: fuwuqiId, jiage: mockPriceMap(fuwuqiId) };
}
