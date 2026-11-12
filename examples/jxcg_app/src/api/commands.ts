// ============================================================================
// API 层：Tauri 命令封装（window.__TAURI__ + withGlobalTauri）
//
// - 桌面运行时（Tauri）直接 invoke Rust 命令，命令名/参数与
//   examples/jxcg_app/src-tauri/src/commands/*.rs 一一对应；
// - 浏览器预览（bun run dev）没有 __TAURI__ 时自动走 ./mock.ts 的 mock 后端；
// - 所有错误统一转成 Error（消息为 Rust 侧中文错误串），由页面 Toast 展示。
// ============================================================================

import type {
  BaobenRequest,
  Catalog,
  JiageJilu,
  JisuanRequest,
  JisuanResult,
  NetRequest,
  NetResult,
  PaihangRequest,
  PaihangRow,
  RefreshProgress,
  RefreshResult,
  Settings,
  Tong,
  WuxingshiLianRequest,
  WuxingshiLianResult,
  ZhuanhuanRequest,
  ZhuanhuanResult,
} from "../types";
import { isMockMode, mockInvoke, mockListen } from "./mock";
import { REFRESH_PROGRESS_EVENT } from "../utils/constants";

export { isMockMode };

type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
type ListenFn = (
  event: string,
  cb: (e: { payload: unknown }) => void,
) => Promise<() => void>;

let _invoke: InvokeFn | null = null;
let _listen: ListenFn | null = null;

function getInvoke(): InvokeFn {
  if (_invoke !== null) return _invoke;
  const api = window.__TAURI__?.core;
  if (api?.invoke) {
    _invoke = api.invoke.bind(api);
    return _invoke;
  }
  const unavailable: InvokeFn = async (command: string) => {
    throw new Error(`Tauri 通道不可用：${command}`);
  };
  _invoke = unavailable;
  return unavailable;
}

function getListen(): ListenFn | null {
  if (_listen !== null) return _listen;
  const evt = window.__TAURI__?.event;
  if (evt?.listen) {
    _listen = evt.listen.bind(evt);
    return _listen;
  }
  _listen = null;
  return null;
}

/** 是否运行在 Tauri 桌面壳内（浏览器预览返回 false）。 */
export function isTauriRuntime(): boolean {
  return !!window.__TAURI__?.core?.invoke;
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    return (await mockInvoke(command, args)) as T;
  }
  try {
    return (await getInvoke()(command, args)) as T;
  } catch (err) {
    // Tauri 抛出的通常是字符串（中文错误串），统一包成 Error 便于 catch/toast。
    throw err instanceof Error ? err : new Error(String(err));
  }
}

/** 冒烟命令：确认前后端通道正常。 */
export async function ping(): Promise<string> {
  return invoke<string>("ping");
}

// ---------------------------------------------------------------------------
// 数据（data）
// ---------------------------------------------------------------------------

/** 加载随包目录（物品/配方/五行石/服务器/值集）。 */
export async function loadCatalog(): Promise<Catalog> {
  return invoke<Catalog>("load_catalog");
}

/** 清缓存并重新读取 data/（数据重新导出后使用；AppState 进程内缓存）。 */
export async function reloadCatalog(): Promise<Catalog> {
  return invoke<Catalog>("reload_catalog");
}

// ---------------------------------------------------------------------------
// 计算（calc）
// ---------------------------------------------------------------------------

/** 交易行净收入（扣手续费 + 保管费）。 */
export async function calculateNet(req: NetRequest): Promise<NetResult> {
  return invoke<NetResult>("calculate_net", { req });
}

/** 保本总售价（铜）。 */
export async function calculateBaoben(req: BaobenRequest): Promise<Tong> {
  return invoke<Tong>("calculate_baoben", { req });
}

/** 单配方成本利润。 */
export async function calculateRecipe(req: JisuanRequest): Promise<JisuanResult> {
  return invoke<JisuanResult>("calculate_recipe", { req });
}

/** 全配方排行。 */
export async function rankRecipes(req: PaihangRequest): Promise<PaihangRow[]> {
  return invoke<PaihangRow[]>("rank_recipes", { req });
}

/** 五行石转换（精炼，旧单步）。 */
export async function calculateConvert(req: ZhuanhuanRequest): Promise<ZhuanhuanResult> {
  return invoke<ZhuanhuanResult>("calculate_convert", { req });
}

/** 五行石一键合成（面板配方链，含合 6 出 8 级期望）。 */
export async function calculateWuxingshi(req: WuxingshiLianRequest): Promise<WuxingshiLianResult> {
  return invoke<WuxingshiLianResult>("calculate_wuxingshi", { req });
}

// ---------------------------------------------------------------------------
// 行情（market）
// ---------------------------------------------------------------------------

/** 读取本地行情缓存（某服）。 */
export async function loadPrices(fuwuqiId: string): Promise<JiageJilu[]> {
  return invoke<JiageJilu[]>("load_prices", { fuwuqiId });
}

/** 手动覆盖一条物品价格（shoudong 优先于自动价）。 */
export async function upsertPrice(
  fuwuqiId: string,
  wupinId: string,
  danjia: Tong,
): Promise<void> {
  await invoke<null>("upsert_price", { fuwuqiId, wupinId, danjia });
}

/** 联网刷新行情（JX3BOX）；进度通过 refresh_progress 事件推送。 */
export async function refreshPrices(
  fuwuqiId: string,
  wupinIds: string[],
): Promise<RefreshResult> {
  return invoke<RefreshResult>("refresh_prices", { fuwuqiId, wupinIds });
}

/**
 * 监听行情刷新进度（事件名 refresh-progress，payload: RefreshProgress）。
 * 浏览器 mock 下同样工作；返回取消监听函数。
 */
export async function listenRefreshProgress(
  cb: (progress: RefreshProgress) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) {
    return mockListen(REFRESH_PROGRESS_EVENT, (event) => cb(event.payload as RefreshProgress));
  }
  const listen = getListen();
  if (!listen) return () => {};
  return listen(REFRESH_PROGRESS_EVENT, (event) => cb(event.payload as RefreshProgress));
}

// ---------------------------------------------------------------------------
// 设置（settings）
// ---------------------------------------------------------------------------

/** 读取本地设置。 */
export async function loadSettings(): Promise<Settings> {
  return invoke<Settings>("load_settings");
}

/** 保存设置到本地存档。 */
export async function saveSettings(settings: Settings): Promise<void> {
  await invoke<null>("save_settings", { settings });
}
