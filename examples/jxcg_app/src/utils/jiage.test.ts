import { describe, expect, it } from "vitest";
import { jiageKey, quanliangJiageMap, shoudongIdsOf } from "./jiage";
import type { JiageBiao } from "../types";

const biao: JiageBiao = {
  [jiageKey("jiandanqinxin", "5_30853")]: { danjia: 8_000, gengxin_shijian: 1, laiyuan: "jx3box", shoudong: false },
  [jiageKey("jiandanqinxin", "5_89970")]: { danjia: 300, gengxin_shijian: 1, laiyuan: "shoudong", shoudong: true },
  [jiageKey("jiandanqinxin", "5_24423")]: { danjia: 9_500, gengxin_shijian: 1, laiyuan: "shoudong", shoudong: true },
  [jiageKey("jiandanqinxin", "5_00000")]: { danjia: 0, gengxin_shijian: 1, laiyuan: "jx3box", shoudong: false },
  [jiageKey("changancheng", "5_30853")]: { danjia: 12_000, gengxin_shijian: 1, laiyuan: "jx3box", shoudong: false },
};

describe("行情全量映射 / 手填 id（task-6 契约）", () => {
  it("quanliangJiageMap 只取当前服务器前缀且 danjia>0", () => {
    const map = quanliangJiageMap(biao, "jiandanqinxin");
    expect(map).toEqual({ "5_30853": 8_000, "5_89970": 300, "5_24423": 9_500 });
    expect(map["5_00000"]).toBeUndefined();
    expect(map["5_30853"]).toBe(8_000);
    // 其它服务器不混入
    expect(quanliangJiageMap(biao, "changancheng")).toEqual({ "5_30853": 12_000 });
  });

  it("shoudongIdsOf 只取当前服务器手填价，排序稳定", () => {
    expect(shoudongIdsOf(biao, "jiandanqinxin")).toEqual(["5_24423", "5_89970"]);
    expect(shoudongIdsOf(biao, "changancheng")).toEqual([]);
    expect(shoudongIdsOf({}, "jiandanqinxin")).toEqual([]);
  });
});

import { danjiaOf } from "./jiage";
import type { Wupin } from "../types";

describe("danjiaOf 取价优先级（task-13：手填 > 商店价 > 行情 > 缺价）", () => {
  const wupin = (id: string, shangdianjia?: number): Wupin => ({
    id,
    mingcheng: id,
    pinlei: "cailiao",
    dui: 1,
    shangdianjia: shangdianjia ?? null,
    laiyuan: null,
  });
  const entry = (danjia: number, shoudong = false) => ({
    danjia,
    gengxin_shijian: 1,
    laiyuan: shoudong ? "shoudong" : "jx3box",
    shoudong,
  });
  const biao: JiageBiao = {
    [jiageKey("jiandanqinxin", "npc_shop")]: entry(100),
    [jiageKey("jiandanqinxin", "npc_shop_manual")]: entry(50, true),
    [jiageKey("jiandanqinxin", "normal")]: entry(200),
  };

  it("商店价优先于行情（即使行情更低/更高）", () => {
    const result = danjiaOf(biao, "jiandanqinxin", wupin("npc_shop", 300));
    expect(result).toEqual({ danjia: 300, laiyuan: "shangdian" });
  });

  it("手填覆盖商店价", () => {
    const result = danjiaOf(biao, "jiandanqinxin", wupin("npc_shop_manual", 300));
    expect(result).toEqual({ danjia: 50, laiyuan: "shoudong" });
  });

  it("无商店价时用行情；都没有返回 null（缺价）", () => {
    expect(danjiaOf(biao, "jiandanqinxin", wupin("normal"))).toEqual({ danjia: 200, laiyuan: "jx3box" });
    expect(danjiaOf(biao, "jiandanqinxin", wupin("missing"))).toBeNull();
  });
});
