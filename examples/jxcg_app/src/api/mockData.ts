// ============================================================================
// 浏览器预览（mock）数据 —— 与 data/*.toml 骨架数据保持一致，
// 并补充少量「mock-preview」示例配方/物品，方便在浏览器里走通四个页面。
//
// Tauri 桌面运行时一律走 Rust 后端（真实 data/ 与行情），不会加载本文件数据。
// ============================================================================

import type { Catalog, Tong, Wupin } from "../types";
import { DEFAULT_SHOUXUFEI } from "../utils/constants";

function wupin(
  id: string,
  mingcheng: string,
  pinlei: Wupin["pinlei"],
  extra: Partial<Wupin> = {},
): Wupin {
  return { id, mingcheng, pinlei, dui: 20, laiyuan: "mock", ...extra };
}

export const MOCK_CATALOG: Catalog = {
  banben: {
    level: 50,
    season: 3,
    modified: 20261112,
    note: "50级·苍生铸世（浏览器 mock 预览数据）· 桌面端以 data/*.toml 为准",
  },
  wupin: [
    // ---- data/wupin.toml 骨架条目 ----
    wupin("5_30853", "珠贝母", "cailiao", { shangdianjia: 1000, laiyuan: "jx3box" }),
    wupin("5_30845", "云芝", "cailiao", { shangdianjia: 1000, laiyuan: "jx3box" }),
    wupin("5_89970", "酱料", "cailiao", { shangdianjia: 1, laiyuan: "jx3box" }),
    wupin("5_89167", "竹沥", "cailiao", { shangdianjia: 20, laiyuan: "jx3box" }),
    wupin("5_31984", "老豆腐", "cailiao", { shangdianjia: 55, laiyuan: "jx3box" }),
    wupin("5_89346", "云芝竹沥羹", "chengpin", { laiyuan: "jx3box" }),
    wupin("5_89347", "云芝竹沥羹·精", "chengpin", { laiyuan: "jx3box" }),
    wupin("5_24423", "五行石（一级）", "wuxingshi", { laiyuan: "jx3box" }),
    wupin("5_24424", "五行石（二级）", "wuxingshi", { laiyuan: "jx3box" }),
    wupin("5_24425", "五行石（三级）", "wuxingshi", { laiyuan: "jx3box" }),
    wupin("5_24426", "五行石（四级）", "wuxingshi", { laiyuan: "jx3box" }),
    wupin("5_24427", "五行石（五级）", "wuxingshi", { laiyuan: "jx3box" }),
    wupin("5_24428", "五行石（六级）", "wuxingshi", { laiyuan: "jx3box" }),
    wupin("5_24429", "五行石（七级）", "wuxingshi", { laiyuan: "jx3box" }),
    wupin("5_24430", "五行石（八级）", "wuxingshi", { laiyuan: "jx3box" }),
    // ---- mock-preview：示例配方材料/产物 ----
    wupin("5_50001", "锦缎", "cailiao", { shangdianjia: 200, laiyuan: "mock-preview" }),
    wupin("5_50002", "粗麻线", "cailiao", { shangdianjia: 30, laiyuan: "mock-preview" }),
    wupin("5_50003", "云锦外袍", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50014", "云锦外袍·精", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50004", "陨铁", "cailiao", { shangdianjia: 500, laiyuan: "mock-preview" }),
    wupin("5_50005", "寒铁护腕", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50018", "寒铁护腕·精", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50006", "上等药材", "cailiao", { shangdianjia: 80, laiyuan: "mock-preview" }),
    wupin("5_50007", "玄参丸", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50012", "玄参丸·精", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50009", "雕花屏风", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50019", "雕花屏风·精", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50015", "糯米", "cailiao", { shangdianjia: 40, laiyuan: "mock-preview" }),
    wupin("5_50016", "桂花糕点", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50017", "桂花糕点·精", "chengpin", { laiyuan: "mock-preview" }),
    wupin("5_50010", "蚕丝", "cailiao", { shangdianjia: 60, laiyuan: "mock-preview" }),
    // 不可获得材料示例：无 NPC 商店价、无行情价、也没有任何配方产出
    wupin("5_50020", "水纹纸", "cailiao", { laiyuan: "mock-preview" }),
    wupin("5_50021", "水墨画", "chengpin", { laiyuan: "mock-preview" }),
  ],
  peifang: [
    // data/peifang.toml：云芝竹沥羹（烹饪，示例）
    {
      id: "cooking_437",
      mingcheng: "云芝竹沥羹",
      jiyi: "pengren",
      dengji: 50,
      jingli: 75,
      zhizuofei: 0,
      laiyuan: "jx3box",
      cailiao: [
        { wupin_id: "5_30853", shuliang: 1 },
        { wupin_id: "5_30845", shuliang: 3 },
        { wupin_id: "5_89970", shuliang: 3 },
        { wupin_id: "5_89167", shuliang: 1 },
        { wupin_id: "5_31984", shuliang: 10 },
      ],
      chuchan: [
        { wupin_id: "5_89346", min: 2, max: 3, quanzhong: 1006633 },
        { wupin_id: "5_89347", min: 2, max: 3, quanzhong: 41943 },
      ],
    },
    // ---- 以下为浏览器预览补充（mock-preview） ----
    {
      id: "zhiyao_88",
      mingcheng: "玄参丸",
      jiyi: "zhiyao",
      dengji: 45,
      jingli: 50,
      zhizuofei: 0,
      laiyuan: "mock-preview",
      cailiao: [
        { wupin_id: "5_50006", shuliang: 5 },
        { wupin_id: "5_89970", shuliang: 2 },
      ],
      chuchan: [
        { wupin_id: "5_50007", min: 1, max: 1, quanzhong: 1006633 },
        { wupin_id: "5_50012", min: 1, max: 1, quanzhong: 41943 },
      ],
    },
    {
      id: "fengren_201",
      mingcheng: "云锦外袍",
      jiyi: "fengren",
      dengji: 50,
      jingli: 80,
      zhizuofei: 2000,
      laiyuan: "mock-preview",
      cailiao: [
        { wupin_id: "5_50001", shuliang: 4 },
        { wupin_id: "5_50002", shuliang: 6 },
        { wupin_id: "5_50010", shuliang: 2 },
      ],
      chuchan: [
        { wupin_id: "5_50003", min: 1, max: 1, quanzhong: 1006633 },
        { wupin_id: "5_50014", min: 1, max: 1, quanzhong: 41943 },
      ],
    },
    {
      id: "zhuzhao_77",
      mingcheng: "寒铁护腕",
      jiyi: "zhuzhao",
      dengji: 45,
      jingli: 120,
      zhizuofei: 0,
      laiyuan: "mock-preview",
      cailiao: [
        { wupin_id: "5_50004", shuliang: 8 },
        { wupin_id: "5_50002", shuliang: 2 },
      ],
      chuchan: [
        { wupin_id: "5_50005", min: 1, max: 1, quanzhong: 1006633 },
        { wupin_id: "5_50018", min: 1, max: 1, quanzhong: 41943 },
      ],
    },
    {
      id: "zijiang_415",
      mingcheng: "雕花屏风",
      jiyi: "zijiang",
      dengji: 40,
      jingli: 100,
      zhizuofei: 0,
      laiyuan: "mock-preview",
      cailiao: [
        { wupin_id: "5_50004", shuliang: 3 },
        { wupin_id: "5_50001", shuliang: 2 },
      ],
      chuchan: [
        { wupin_id: "5_50009", min: 1, max: 1, quanzhong: 1006633 },
        { wupin_id: "5_50019", min: 1, max: 1, quanzhong: 41943 },
      ],
    },
    {
      id: "cooking_512",
      mingcheng: "桂花糕点",
      jiyi: "pengren",
      dengji: 35,
      jingli: 30,
      zhizuofei: 0,
      laiyuan: "mock-preview",
      cailiao: [
        { wupin_id: "5_50015", shuliang: 5 },
        { wupin_id: "5_89970", shuliang: 3 },
      ],
      chuchan: [
        { wupin_id: "5_50016", min: 2, max: 2, quanzhong: 1006633 },
        { wupin_id: "5_50017", min: 2, max: 2, quanzhong: 41943 },
      ],
    },
    // ---- 中间产物制作配方（产物同时是其它配方的材料）----
    // 酱料：cooking_437 / zhiyao_88 / cooking_512 都需要它
    {
      id: "tiaoliao_1",
      mingcheng: "酱料制作",
      jiyi: "pengren",
      dengji: 20,
      jingli: 10,
      zhizuofei: 0,
      laiyuan: "mock-preview",
      cailiao: [
        { wupin_id: "5_50015", shuliang: 5 },
        { wupin_id: "5_31984", shuliang: 2 },
      ],
      chuchan: [{ wupin_id: "5_89970", min: 5, max: 5, quanzhong: 1048576 }],
    },
    // 水墨画：用到「不可获得材料」水纹纸，用于演示 bukede 分类
    {
      id: "shuimo_1",
      mingcheng: "水墨画",
      jiyi: "zijiang",
      dengji: 35,
      jingli: 40,
      zhizuofei: 0,
      laiyuan: "mock-preview",
      cailiao: [
        { wupin_id: "5_50020", shuliang: 2 },
        { wupin_id: "5_50001", shuliang: 1 },
      ],
      chuchan: [{ wupin_id: "5_50021", min: 1, max: 1, quanzhong: 1048576 }],
    },
    // 锦缎：fengren_201 / zijiang_415 都需要它
    {
      id: "buzhi_1",
      mingcheng: "锦缎织造",
      jiyi: "fengren",
      dengji: 25,
      jingli: 20,
      zhizuofei: 1000,
      laiyuan: "mock-preview",
      cailiao: [
        { wupin_id: "5_50002", shuliang: 4 },
        { wupin_id: "5_50010", shuliang: 1 },
      ],
      chuchan: [{ wupin_id: "5_50001", min: 2, max: 2, quanzhong: 1048576 }],
    },
  ],
  // data/wuxingshi.toml：旧单步转换（保留兼容；新 UI 走 fangan + hecheng）
  wuxingshi: [
    {
      id: "wuxing_1to3",
      mingcheng: "一级→三级（12 合 1）",
      cong: "5_24423",
      dao: "5_24425",
      xuyao: 12,
      jingli: 10,
      jinbi: 3600,
      chenggonglv_bp: 9276,
      laiyuan: "面板-2026-11",
    },
    {
      id: "wuxing_3to4",
      mingcheng: "三级+一级→四级",
      cong: "5_24425",
      dao: "5_24426",
      xuyao: 3,
      jingli: 10,
      jinbi: 14421,
      chenggonglv_bp: 9911,
      laiyuan: "面板-2026-11",
    },
  ],
  // data/wuxingshi.toml：五行石合成方案（[[fangan]]）
  wuxingshi_fangan: [
    {
      id: "mianban",
      mingcheng: "面板配方（游戏内一键合成）",
      note: "3级=12一级；4级=3三级+7一级；5级=3四级+2三级；6级=3五级+2四级；概率取自 2026-11 精炼面板",
    },
  ],
  // data/wuxingshi.toml：单步合成配方（[[hecheng]]，jieguo.bp 万分比）
  wuxingshi_hecheng: [
    {
      id: "mianban_3",
      fangan: "mianban",
      chanpin: "5_24425",
      jingli: 10,
      jiagongfei: 3600,
      cailiao: [{ wupin_id: "5_24423", shuliang: 12 }],
      jieguo: [
        { wupin_id: "5_24425", bp: 9276 },
        { wupin_id: "5_24424", bp: 724 },
      ],
    },
    {
      id: "mianban_4",
      fangan: "mianban",
      chanpin: "5_24426",
      jingli: 10,
      jiagongfei: 14421,
      cailiao: [
        { wupin_id: "5_24425", shuliang: 3 },
        { wupin_id: "5_24423", shuliang: 7 },
      ],
      jieguo: [
        { wupin_id: "5_24426", bp: 9911 },
        { wupin_id: "5_24427", bp: 72 },
      ],
    },
    {
      id: "mianban_5",
      fangan: "mianban",
      chanpin: "5_24427",
      jingli: 10,
      jiagongfei: 53801,
      cailiao: [
        { wupin_id: "5_24426", shuliang: 3 },
        { wupin_id: "5_24425", shuliang: 2 },
      ],
      jieguo: [
        { wupin_id: "5_24427", bp: 9915 },
        { wupin_id: "5_24428", bp: 85 },
      ],
    },
    {
      id: "mianban_6",
      fangan: "mianban",
      chanpin: "5_24428",
      jingli: 10,
      jiagongfei: 199066,
      cailiao: [
        { wupin_id: "5_24427", shuliang: 3 },
        { wupin_id: "5_24426", shuliang: 2 },
      ],
      jieguo: [
        { wupin_id: "5_24428", bp: 9955 },
        // 合 6 意外出 8 级：面板概率待校准，bp=0（前端 baoji_bp 覆盖）
        { wupin_id: "5_24430", bp: 0 },
      ],
    },
  ],
  // data/fuwuqi.toml
  fuwuqi: [
    { id: "jiandanqinxin", mingcheng: "剑胆琴心", daju: "电信五区" },
    { id: "changancheng", mingcheng: "长安城", daju: "电信一区" },
    { id: "weiwoduzun", mingcheng: "唯我独尊", daju: "电信五区" },
    { id: "shanhaixiangfeng", mingcheng: "山海相逢", daju: "无界区" },
    { id: "meijianxue", mingcheng: "眉间雪", daju: "无界区" },
  ],
  // data/values.toml
  zhiji: [
    {
      id: "live-130",
      mingcheng: "正式服·50级",
      note: "5% 四舍五入 + 保管费 80 银/24h（社区口径，实机校准后更新）",
      shouxufei: { ...DEFAULT_SHOUXUFEI },
    },
    {
      id: "debug-0",
      mingcheng: "调试值集·免手续费",
      note: "仅用于验证计算链路：0% 手续费 + 无保管费",
      shouxufei: { shuilv_bp: 0, quzheng: "half_up", baoguanfei: 0 },
    },
  ],
};

/** 浏览器预览的基础行情（铜/件）；缺省表示该物品「缺价」（演示缺价过滤）。 */
export const MOCK_BASE_JIAGE: Record<string, Tong> = {
  "5_30853": 8_000,
  "5_30845": 12_000,
  "5_89970": 300,
  "5_89167": 9_000,
  "5_31984": 300,
  "5_89346": 98_000,
  "5_89347": 168_000,
  "5_24423": 9_500,
  "5_24424": 24_000,
  "5_24425": 78_000,
  "5_24426": 420_000,
  "5_24427": 1_100_000,
  "5_24428": 6_000_000,
  "5_24429": 4_000_000,
  "5_24430": 255_000_000,
  "5_50001": 5_200,
  "5_50002": 900,
  "5_50003": 78_000,
  "5_50014": 108_000,
  "5_50004": 12_000,
  "5_50005": 168_000,
  "5_50018": 260_000,
  "5_50006": 6_500,
  "5_50009": 128_000,
  "5_50019": 208_000,
  "5_50015": 800,
  "5_50016": 26_000,
  "5_50017": 46_000,
  "5_50010": 12_000,
  "5_50021": 120_000,
  // 5_50020 水纹纸：故意无行情价（无 NPC 商店价、无配方产出）
  // 5_50007 玄参丸 / 5_50012 玄参丸·精 故意不给价 → 演示缺价
};

/** 各服价格系数（浏览器预览用：不同服价格略有差异）。 */
export const MOCK_SERVER_FACTOR: Record<string, number> = {
  jiandanqinxin: 1,
  changancheng: 1.06,
  weiwoduzun: 0.97,
  shanhaixiangfeng: 1.03,
  meijianxue: 0.94,
};
