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
