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
  it("云芝竹沥羹：材料成本 569 金、期望产出 2.5 件、无缺价", async () => {
    const result = (await mockInvoke("calculate_recipe", { req: recipeRequest() })) as {
      chengben_cailiao: number;
      chuchan_qiwang: number;
      lirun: number;
      quejia: string[];
      cailiao: Array<{ wupin_id: string; xiaojia: number }>;
    };
    // 1×80000? 注意 mockPriceMap 基准价：珠贝母 8000、云芝 12000、酱料 300、竹沥 9000、老豆腐 300
    expect(result.chengben_cailiao).toBe(8_000 + 3 * 12_000 + 3 * 300 + 9_000 + 10 * 300);
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

  it("五行石 3→4：成功率 48.78% 按 1/p 期望次数计成本", async () => {
    const result = (await mockInvoke("calculate_convert", {
      req: {
        zhuanhuan_id: "wuxing_3to4",
        cishu: 1,
        jingli_danjia: 0,
        shouxufei,
        jiage: mockPriceMap("jiandanqinxin"),
      },
    })) as { qiwang_cishu: number; zong_chengben: number; quejia: string[] };
    expect(result.qiwang_cishu).toBeCloseTo(1 / 0.4878, 4);
    expect(result.zong_chengben).toBeGreaterThan(0);
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
    // 酱料单位自制成本 = (糯米 5×800 + 老豆腐 2×300) / 5件 = 920 铜
    expect(jiangliao.zhongjian).toBe(true);
    expect(jiangliao.laiyuan).toBe("zizhi");
    expect(jiangliao.danjia).toBe(920);
    expect(jiangliao.xiaojia).toBe(920 * 3);
    // 材料成本含自制行：8000 + 36000 + 2760 + 9000 + 3000
    expect(result.chengben_cailiao).toBe(58_760);
    expect(result.chengben_zhongjian).toBe(2_760);
    // 中间产物精力：10 点/次 ÷ 5 件 × 3 件 = 6 点
    expect(result.jingli_zhongjian).toBe(6);
    expect(result.zhongjian).toHaveLength(1);
    const zj = result.zhongjian[0];
    expect(zj.peifang_id).toBe("tiaoliao_1");
    expect(zj.shuliang).toBe(3);
    expect(zj.danjia).toBe(920);
    expect(zj.jingli).toBe(6);
    // 展开的采购原料来自全量行情映射（糯米 800 / 老豆腐 300）
    const nuomi = zj.cailiao.find((row) => row.wupin_id === "5_50015")!;
    expect(nuomi.danjia).toBe(800);
    expect(nuomi.laiyuan).toBe("hangqing");
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
    // 旧口径材料成本：8000 + 36000 + 900 + 9000 + 3000
    expect(result.chengben_cailiao).toBe(56_900);
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
