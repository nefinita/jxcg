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
  RefreshProgress,
  RefreshResult,
  Settings,
  Shouxufei,
  Tong,
  ZhuanhuanRequest,
  ZhuanhuanResult,
} from "../types";
import { QUANZHONG_FENMU } from "../types";
import { DEFAULT_SETTINGS, REFRESH_PROGRESS_EVENT } from "../utils/constants";
import { nowSeconds } from "../utils/format";
import { MOCK_BASE_JIAGE, MOCK_CATALOG, MOCK_SERVER_FACTOR } from "./mockData";

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

/** 材料/产物单价：行情 > 商店价 > 缺价。 */
function chuchanDanjia(
  catalog: Catalog,
  jiage: Record<string, Tong>,
  wupinId: string,
  quejia: string[],
): { danjia: Tong; laiyuan: string } {
  const jiageValue = jiage[wupinId];
  if (typeof jiageValue === "number" && jiageValue > 0) {
    return { danjia: jiageValue, laiyuan: "hangqing" };
  }
  const wupin = findWupin(catalog, wupinId);
  if (wupin?.shangdianjia && wupin.shangdianjia > 0) {
    return { danjia: wupin.shangdianjia, laiyuan: "shangdian" };
  }
  if (!quejia.includes(wupinId)) quejia.push(wupinId);
  return { danjia: 0, laiyuan: "quejia" };
}

function mockCalculateRecipe(req: JisuanRequest): JisuanResult {
  const peifang = MOCK_CATALOG.peifang.find((p) => p.id === req.peifang_id);
  if (!peifang) throw new Error(`配方不存在：${req.peifang_id}`);
  const cishu = Math.max(1, Math.trunc(req.cishu));
  const quejia: string[] = [];

  const cailiao: CailiaoMingxi[] = [];
  let chengbenCailiao: Tong = 0;
  for (const item of peifang.cailiao) {
    const need = item.shuliang * cishu;
    const { danjia, laiyuan } = chuchanDanjia(MOCK_CATALOG, req.jiage, item.wupin_id, quejia);
    const xiaojia = danjia * need;
    chengbenCailiao += xiaojia;
    cailiao.push({
      wupin_id: item.wupin_id,
      mingcheng: findWupin(MOCK_CATALOG, item.wupin_id)?.mingcheng ?? item.wupin_id,
      shuliang: item.shuliang,
      danjia,
      xiaojia,
      laiyuan,
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
    const { danjia } = chuchanDanjia(MOCK_CATALOG, req.jiage, slot.wupin_id, quejia);
    shouru += Math.round(qiwang * danjia);
    chuchan.push({
      wupin_id: slot.wupin_id,
      mingcheng: findWupin(MOCK_CATALOG, slot.wupin_id)?.mingcheng ?? slot.wupin_id,
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
  const meiJingli = jingli > 0 ? Math.trunc(lirun / jingli) : lirun;
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
    quejia: [...new Set(quejia)].sort(),
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
  for (const peifang of MOCK_CATALOG.peifang) {
    if (req.jiyi && peifang.jiyi !== req.jiyi) continue;
    const result = mockCalculateRecipe({
      peifang_id: peifang.id,
      cishu: 1,
      guadan_cishu: 1,
      jingli_danjia: req.jingli_danjia,
      shouxufei: req.shouxufei,
      jiage: req.jiage,
      jiyi_dengji: req.jiyi_dengji,
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
  const conga = req.jiage[zhuanhuan.cong] ?? 0;
  const daojia = req.jiage[zhuanhuan.dao] ?? 0;
  if (!conga) quejia.push(zhuanhuan.cong);
  if (!daojia) quejia.push(zhuanhuan.dao);
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
