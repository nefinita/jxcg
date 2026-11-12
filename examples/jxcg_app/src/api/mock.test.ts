// 浏览器 mock 引擎与 Rust 金标准口径的一致性回归（金额单位：铜）
import { beforeEach, describe, expect, it } from "vitest";
import { mockCatalog, mockInvoke, mockPriceMap } from "./mock";
import type { JiageBiao, JisuanRequest, NetRequest, PaihangRequest, Shouxufei } from "../types";

const shouxufei: Shouxufei = { shuilv_bp: 500, quzheng: "half_up", baoguanfei: 8_000 };

beforeEach(() => {
  localStorage.clear();
});

function priceBiao(fuwuqiId = "jiandanqinxin"): JiageBiao {
  const biao: JiageBiao = {};
  for (const [wupinId, danjia] of Object.entries(mockPriceMap(fuwuqiId))) {
    biao[`${fuwuqiId}|${wupinId}`] = {
      danjia,
      gengxin_shijian: 1,
      laiyuan: "jx3box",
      shoudong: false,
    };
  }
  return biao;
}

function recipeRequest(overrides: Partial<JisuanRequest> = {}): JisuanRequest {
  return {
    peifang_id: "cooking_437",
    cishu: 1,
    guadan_cishu: 1,
    jingli_danjia: 0,
    shouxufei,
    jiage: mockPriceMap("jiandanqinxin"),
    shoudong: [],
    jiyi_dengji: 0,
    // 默认按旧口径（不展开中间产物），便于断言原始行情价；展开口径单独测试。
    zhongjian_zizhi: false,
    ...overrides,
  };
}

describe("mock 净收入 / 保本价（PLAN §6.2 / §6.4）", () => {
  it("1200 金成交：手续费 60 金、保管费 80 银、到手 1139 金 20 银", async () => {
    const req: NetRequest = { shoujia: 12_000_000, shuliang: 1, shouxufei, guadan_cishu: 1 };
    const result = (await mockInvoke("calculate_net", { req })) as {
      zonge: number;
      feiyong: number;
      baoguanfei: number;
      daoshou: number;
    };
    expect(result.zonge).toBe(12_000_000);
    expect(result.feiyong).toBe(600_000);
    expect(result.baoguanfei).toBe(8_000);
    expect(result.daoshou).toBe(11_392_000);
  });

  it("5% HALF_UP 边界：10 铜四舍五入为 1 铜", async () => {
    for (const [danjia, expected] of [
      [10, 1],
      [190, 10], // 9.5 → 10（HALF_UP）
      [100, 5],
    ] as const) {
      const req: NetRequest = { shoujia: danjia, shuliang: 1, shouxufei, guadan_cishu: 0 };
      const result = (await mockInvoke("calculate_net", { req })) as { feiyong: number };
      expect(result.feiyong).toBe(expected);
    }
  });

  it("保本价是最小售价：保本价净收入 >= 成本，减 1 铜则不足", async () => {
    const chengben = 10_000_000;
    const p = (await mockInvoke("calculate_baoben", {
      req: { chengben, shouxufei, guadan_cishu: 1 },
    })) as number;
    const net = async (shoujia: number) => {
      const result = (await mockInvoke("calculate_net", {
        req: { shoujia, shuliang: 1, shouxufei, guadan_cishu: 1 },
      })) as { daoshou: number };
      // 保本口径：净收入（扣手续费）− 保管费 >= 成本
      return result.daoshou;
    };
    expect(await net(p)).toBeGreaterThanOrEqual(chengben);
    expect(await net(p - 1)).toBeLessThan(chengben);
  });
});

describe("mock 配方成本 / 排行 / 五行石", () => {
  it("云芝竹沥羹：NPC 商店价材料成本 45.73 金、期望产出 2.5 件、无缺价", async () => {
    const result = (await mockInvoke("calculate_recipe", { req: recipeRequest() })) as {
      chengben_cailiao: number;
      chuchan_qiwang: number;
      lirun: number;
      quejia: string[];
      cailiao: Array<{ wupin_id: string; xiaojia: number }>;
    };
    // 商店价优先：珠贝母 1000 + 云芝 3×1000 + 酱料 3×1 + 竹沥 20 + 老豆腐 10×55 = 4573
    expect(result.chengben_cailiao).toBe(1_000 + 3 * 1_000 + 3 * 1 + 20 + 10 * 55);
    expect(result.chuchan_qiwang).toBeCloseTo(2.5, 6);
    expect(result.quejia).toEqual([]);
    expect(result.lirun).toBeGreaterThan(0);
  });

  it("缺价配方默认不参与排行，「包含缺价配方」时排在末尾", async () => {
    const base: PaihangRequest = {
      jiyi: null,
      jingli_danjia: 0,
      shouxufei,
      jiage: mockPriceMap("jiandanqinxin"),
      zhibiao: "lirun",
      jiyi_dengji: 0,
      limit: 20,
      shoudong: [],
      zhongjian_zizhi: true,
      baohan_zhongjian: false,
    };
    const withoutQuejia = (await mockInvoke("rank_recipes", { req: base })) as Array<{
      peifang_id: string;
      quejia: boolean;
    }>;
    expect(withoutQuejia.length).toBeGreaterThan(0);
    expect(withoutQuejia.some((row) => row.quejia)).toBe(false);
    expect(withoutQuejia.some((row) => row.peifang_id === "zhiyao_88")).toBe(false);
    // 指标降序
    const profits = withoutQuejia.map((row) => (row as unknown as { lirun: number }).lirun);
    expect([...profits].sort((a, b) => b - a)).toEqual(profits);

    const withQuejia = (await mockInvoke("rank_recipes", {
      req: { ...base, baohan_quejia: true },
    })) as Array<{ peifang_id: string; quejia: boolean }>;
    expect(withQuejia.some((row) => row.peifang_id === "zhiyao_88" && row.quejia)).toBe(true);
    expect(withQuejia[withQuejia.length - 1].quejia).toBe(true);
  });

  it("五行石 3→4（面板 99.11%）：按 1/p 期望次数计成本", async () => {
    const result = (await mockInvoke("calculate_convert", {
      req: {
        zhuanhuan_id: "wuxing_3to4",
        cishu: 1,
        jingli_danjia: 0,
        shouxufei,
        jiage: mockPriceMap("jiandanqinxin"),
      },
    })) as { qiwang_cishu: number; zong_chengben: number; shouru: number; quejia: string[] };
    expect(result.qiwang_cishu).toBeCloseTo(1 / 0.9911, 6);
    // 单次投入 = 3×三级(78000) + 14421 = 248421；期望成本 = round(248421 / 0.9911)
    expect(result.zong_chengben).toBe(250_652);
    expect(result.shouru).toBe(416_262);
    expect(result.quejia).toEqual([]);
  });

  it("目录与行情 mock 覆盖：物品全量、缺价物品不出现", async () => {
    const catalog = mockCatalog();
    expect(catalog.peifang.length).toBeGreaterThanOrEqual(5);
    expect(catalog.wupin.length).toBeGreaterThanOrEqual(20);
    const jiage = mockPriceMap("jiandanqinxin");
    expect(jiage["5_89346"]).toBe(98_000);
    expect(jiage["5_50007"]).toBeUndefined();
    expect(priceBiao()["jiandanqinxin|5_30853"].danjia).toBe(8_000);
  });
});

describe("中间产物自制成本展开（task-6 口径）", () => {
  it("默认展开中间产物：酱料按自制成本摊入，精力单独统计", async () => {
    const result = (await mockInvoke("calculate_recipe", {
      req: recipeRequest({ zhongjian_zizhi: true }),
    })) as {
      chengben_cailiao: number;
      chengben_zhongjian: number;
      jingli_zhongjian: number;
      cailiao: Array<{
        wupin_id: string;
        danjia: number;
        xiaojia: number;
        laiyuan: string;
        zhongjian: boolean;
      }>;
      zhongjian: Array<{
        wupin_id: string;
        peifang_id: string;
        shuliang: number;
        danjia: number;
        xiaojia: number;
        jingli: number;
        cailiao: Array<{ wupin_id: string; shuliang: number; danjia: number; laiyuan: string }>;
      }>;
      mei_jingli: number;
      lirun: number;
      jingli: number;
    };
    const jiangliao = result.cailiao.find((row) => row.wupin_id === "5_89970")!;
    // 酱料单位自制成本 = (糯米 5×40 + 老豆腐 2×55) / 5件 = 62 铜（NPC 商店价）
    expect(jiangliao.zhongjian).toBe(true);
    expect(jiangliao.laiyuan).toBe("zizhi");
    expect(jiangliao.danjia).toBe(62);
    expect(jiangliao.xiaojia).toBe(62 * 3);
    // 材料成本含自制行：1000 + 3000 + 186 + 20 + 550
    expect(result.chengben_cailiao).toBe(4_756);
    expect(result.chengben_zhongjian).toBe(186);
    // 中间产物精力：10 点/次 ÷ 5 件 × 3 件 = 6 点
    expect(result.jingli_zhongjian).toBe(6);
    expect(result.zhongjian).toHaveLength(1);
    const zj = result.zhongjian[0];
    expect(zj.peifang_id).toBe("tiaoliao_1");
    expect(zj.shuliang).toBe(3);
    expect(zj.danjia).toBe(62);
    expect(zj.jingli).toBe(6);
    // 展开的采购原料：糯米/老豆腐都是 NPC 有售 → 商店价优先（40 / 55）
    const nuomi = zj.cailiao.find((row) => row.wupin_id === "5_50015")!;
    expect(nuomi.danjia).toBe(40);
    expect(nuomi.laiyuan).toBe("shangdian");
    expect(nuomi.shuliang).toBe(3);
    // mei_jingli 按 (最终精力 75 + 中间产物精力 6) 计算
    expect(result.mei_jingli).toBe(Math.trunc(result.lirun / (result.jingli + 6)));
  });

  it("手填价物品不展开：直接用手填单价，中间产物明细为空", async () => {
    const result = (await mockInvoke("calculate_recipe", {
      req: recipeRequest({
        zhongjian_zizhi: true,
        shoudong: ["5_89970"],
        jiage: { ...mockPriceMap("jiandanqinxin"), "5_89970": 300 },
      }),
    })) as {
      chengben_cailiao: number;
      chengben_zhongjian: number;
      jingli_zhongjian: number;
      zhongjian: unknown[];
      cailiao: Array<{ wupin_id: string; danjia: number; laiyuan: string; zhongjian: boolean }>;
    };
    const jiangliao = result.cailiao.find((row) => row.wupin_id === "5_89970")!;
    expect(jiangliao.laiyuan).toBe("shoudong");
    expect(jiangliao.zhongjian).toBe(false);
    expect(jiangliao.danjia).toBe(300);
    expect(result.chengben_zhongjian).toBe(0);
    expect(result.jingli_zhongjian).toBe(0);
    expect(result.zhongjian).toEqual([]);
    // 手填 300 覆盖商店价 1：1000 + 3000 + 900 + 20 + 550
    expect(result.chengben_cailiao).toBe(5_470);
  });

  it("排行默认排除只产中间产物的配方，勾选后返回并打标", async () => {
    const base: PaihangRequest = {
      jiyi: null,
      jingli_danjia: 0,
      shouxufei,
      jiage: mockPriceMap("jiandanqinxin"),
      shoudong: [],
      zhibiao: "lirun",
      jiyi_dengji: 0,
      limit: 50,
      zhongjian_zizhi: true,
      baohan_zhongjian: false,
    };
    const without = (await mockInvoke("rank_recipes", { req: base })) as Array<{
      peifang_id: string;
      zhongjian: boolean;
    }>;
    expect(without.some((row) => row.peifang_id === "tiaoliao_1")).toBe(false);
    expect(without.some((row) => row.peifang_id === "buzhi_1")).toBe(false);
    expect(without.some((row) => row.zhongjian)).toBe(false);

    const withZj = (await mockInvoke("rank_recipes", {
      req: { ...base, baohan_zhongjian: true },
    })) as Array<{ peifang_id: string; zhongjian: boolean }>;
    expect(withZj.some((row) => row.peifang_id === "tiaoliao_1" && row.zhongjian)).toBe(true);
    expect(withZj.some((row) => row.peifang_id === "buzhi_1" && row.zhongjian)).toBe(true);
    // 手工排除后，其它成品配方仍在
    expect(withZj.some((row) => row.peifang_id === "cooking_437")).toBe(true);
  });
});

describe("五行石一键合成到六级（task-10 金标准）", () => {
  const lianReq = (overrides: Partial<Record<string, unknown>> = {}) => ({
    fangan_id: "mianban",
    mubiao_wupin_id: "5_24428",
    shuliang: 1,
    jingli_danjia: 0,
    shouxufei,
    jiage: mockPriceMap("jiandanqinxin"),
    baoji_bp: 0,
    ...overrides,
  });

  it("面板链金标准：约 593 一级石 / 581 精力 / 67.97 金加工费", async () => {
    const result = (await mockInvoke("calculate_wuxingshi", { req: lianReq() })) as {
      cailiao: Array<{ wupin_id: string; shuliang: number; danjia: number }>;
      jingli: number;
      jiagongfei: number;
      cailiao_chengben: number;
      fuchanpin: Array<{ wupin_id: string; qiwang: number }>;
      fuchanpin_shouru: number;
      zong_chengben: number;
      danjia: number;
      shichangjia: number;
      lirun: number;
      buzhou: Array<{ hecheng_id: string; cishu: number }>;
      quejia: string[];
    };
    const stone = result.cailiao.find((row) => row.wupin_id === "5_24423")!;
    expect(stone.shuliang).toBe(593);
    expect(stone.danjia).toBe(9_500);
    expect(result.jingli).toBe(581); // 金标准 581 精力
    expect(result.jiagongfei).toBe(679_704); // 67.97 金
    expect(result.buzhou.map((row) => row.hecheng_id)).toEqual([
      "mianban_3",
      "mianban_4",
      "mianban_5",
      "mianban_6",
    ]);
    // 副产物：2/5/6 级（8 级概率 0 不出现）
    const ids = result.fuchanpin.map((row) => row.wupin_id);
    expect(ids).toEqual(["5_24424", "5_24427", "5_24428"]);
    expect(result.fuchanpin_shouru).toBe(318_408);
    expect(result.zong_chengben).toBe(5_994_348);
    expect(result.danjia).toBe(5_994_348);
    expect(result.shichangjia).toBe(6_000_000);
    expect(result.lirun).toBe(-294_348);
    expect(result.quejia).toEqual([]);
  });

  it("合 6 出 8 级概率改变期望收益（bp=5 ≈ +12.8 金/颗）", async () => {
    const without = (await mockInvoke("calculate_wuxingshi", { req: lianReq({ baoji_bp: 0 }) })) as {
      lirun: number;
      fuchanpin: Array<{ wupin_id: string; qiwang: number; shouru: number; danjia: number }>;
    };
    const withBp = (await mockInvoke("calculate_wuxingshi", { req: lianReq({ baoji_bp: 5 }) })) as {
      lirun: number;
      fuchanpin: Array<{ wupin_id: string; qiwang: number; shouru: number; danjia: number }>;
    };
    const eight = withBp.fuchanpin.find((row) => row.wupin_id === "5_24430")!;
    expect(eight.danjia).toBe(255_000_000); // 八级约 25500 金
    expect(eight.qiwang).toBeCloseTo((5 / 10_000) / 0.9955, 6);
    expect(withBp.lirun - without.lirun).toBe(128_076); // ≈ 12.8 金/颗
    expect(without.fuchanpin.some((row) => row.wupin_id === "5_24430")).toBe(false);
  });

  it("数量翻倍成本线性翻倍；缺价物品进 quejia", async () => {
    const one = (await mockInvoke("calculate_wuxingshi", { req: lianReq() })) as {
      zong_chengben: number;
      danjia: number;
      cailiao: Array<{ shuliang: number }>;
    };
    const two = (await mockInvoke("calculate_wuxingshi", { req: lianReq({ shuliang: 2 }) })) as {
      zong_chengben: number;
      danjia: number;
      cailiao: Array<{ shuliang: number }>;
    };
    expect(two.zong_chengben).toBe(one.zong_chengben * 2);
    expect(two.danjia).toBe(one.danjia);
    expect(two.cailiao[0].shuliang).toBe(one.cailiao[0].shuliang * 2);

    const jiage = { ...mockPriceMap("jiandanqinxin") };
    delete jiage["5_24423"];
    delete jiage["5_24430"];
    const missing = (await mockInvoke("calculate_wuxingshi", {
      req: lianReq({ jiage, baoji_bp: 5 }),
    })) as { quejia: string[]; cailiao: Array<{ wupin_id: string; danjia: number }> };
    expect(missing.quejia).toEqual(["5_24423", "5_24430"]);
    expect(missing.cailiao.find((row) => row.wupin_id === "5_24423")!.danjia).toBe(0);
  });

  it("目标切到五级：只走 3/4/5 三步", async () => {
    const result = (await mockInvoke("calculate_wuxingshi", {
      req: lianReq({ mubiao_wupin_id: "5_24427" }),
    })) as { mubiao_mingcheng: string; buzhou: Array<{ hecheng_id: string }>; jingli: number };
    expect(result.mubiao_mingcheng).toBe("五行石（五级）");
    expect(result.buzhou.map((row) => row.hecheng_id)).toEqual(["mianban_3", "mianban_4", "mianban_5"]);
    expect(result.jingli).toBe(161);
  });
});

describe("NPC 商店价优先（task-13 口径）", () => {
  it("商店价物品即使行情更低也用商店价", async () => {
    const jiage = { ...mockPriceMap("jiandanqinxin"), "5_30853": 10 }; // 行情 10 铜 << 商店 1000
    const result = (await mockInvoke("calculate_recipe", {
      req: recipeRequest({ zhongjian_zizhi: false, jiage }),
    })) as {
      chengben_cailiao: number;
      cailiao: Array<{ wupin_id: string; danjia: number; laiyuan: string }>;
    };
    const row = result.cailiao.find((item) => item.wupin_id === "5_30853")!;
    expect(row.danjia).toBe(1_000);
    expect(row.laiyuan).toBe("shangdian");
    // 行情 10 被忽略：成本与商店价口径一致
    expect(result.chengben_cailiao).toBe(4_573);
  });

  it("手填价可以覆盖 NPC 商店价", async () => {
    const jiage = { ...mockPriceMap("jiandanqinxin"), "5_30853": 10 };
    const result = (await mockInvoke("calculate_recipe", {
      req: recipeRequest({ zhongjian_zizhi: true, jiage, shoudong: ["5_30853"] }),
    })) as {
      chengben_cailiao: number;
      cailiao: Array<{ wupin_id: string; danjia: number; laiyuan: string; zhongjian: boolean }>;
    };
    const row = result.cailiao.find((item) => item.wupin_id === "5_30853")!;
    expect(row.danjia).toBe(10);
    expect(row.laiyuan).toBe("shoudong");
    expect(row.zhongjian).toBe(false);
    // 1000→10 后成本减少 990；酱料按自制展开多 183（1×3 → 62×3）
    expect(result.chengben_cailiao).toBe(4_573 - 990 + 183);
  });

  it("排行同样按商店价计算（行情更低也不影响利润）", async () => {
    const base: PaihangRequest = {
      jiyi: null,
      jingli_danjia: 0,
      shouxufei,
      jiage: mockPriceMap("jiandanqinxin"),
      shoudong: [],
      zhibiao: "lirun",
      jiyi_dengji: 0,
      limit: 50,
      zhongjian_zizhi: false,
      baohan_zhongjian: false,
    };
    const normalRows = (await mockInvoke("rank_recipes", { req: base })) as Array<{
      peifang_id: string;
      lirun: number;
    }>;
    const normal = normalRows.find((row) => row.peifang_id === "cooking_437")!;
    // 行情把珠贝母压到 10 铜：商店价口径下排行利润应完全不变
    const lowRows = (await mockInvoke("rank_recipes", {
      req: { ...base, jiage: { ...base.jiage, "5_30853": 10 } },
    })) as Array<{ peifang_id: string; lirun: number }>;
    const low = lowRows.find((row) => row.peifang_id === "cooking_437")!;
    expect(low.lirun).toBe(normal.lirun);
  });
});

describe("单件成本 / 不可获得材料（task-16）", () => {
  it("单件成本 = 总成本 ÷ 期望产出（HALF_UP，多产出配方已除 2.5）", async () => {
    const result = (await mockInvoke("calculate_recipe", { req: recipeRequest() })) as {
      zong_chengben: number;
      chuchan_qiwang: number;
      danjia_chengben: number;
    };
    expect(result.chuchan_qiwang).toBeCloseTo(2.5, 6);
    expect(result.danjia_chengben).toBe(Math.round(result.zong_chengben / 2.5));
    expect(Math.abs(result.danjia_chengben * 2.5 - result.zong_chengben)).toBeLessThanOrEqual(1);
  });

  it("单产出配方：单件成本 ≈ 总成本", async () => {
    const result = (await mockInvoke("calculate_recipe", {
      req: recipeRequest({ peifang_id: "zhuzhao_77" }),
    })) as { zong_chengben: number; chuchan_qiwang: number; danjia_chengben: number };
    expect(result.chuchan_qiwang).toBeCloseTo(1, 6);
    expect(result.danjia_chengben).toBe(result.zong_chengben);
  });

  it("bukede：水纹纸这类无价且无配方产出的材料单列", async () => {
    const result = (await mockInvoke("calculate_recipe", {
      req: recipeRequest({ peifang_id: "shuimo_1" }),
    })) as { quejia: string[]; bukede: string[] };
    expect(result.quejia).toEqual(["5_50020"]);
    expect(result.bukede).toEqual(["5_50020"]); // 水纹纸：无商店价/行情价/产出配方

    // 缺价的「产物」（5_50007/5_50012 由 zhiyao_88 产出）不算不可获得
    const zhiyao = (await mockInvoke("calculate_recipe", {
      req: recipeRequest({ peifang_id: "zhiyao_88" }),
    })) as { quejia: string[]; bukede: string[] };
    expect(zhiyao.quejia).toContain("5_50007");
    expect(zhiyao.bukede).toEqual([]);
  });
});
