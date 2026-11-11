import { describe, expect, it } from "vitest";
import { parsePastePrices } from "./marketPaste";
import { mockCatalog } from "../api/mock";

const wupin = mockCatalog().wupin;

describe("行情粘贴导入解析", () => {
  it("按物品 id / 名称解析，忽略注释与空行", () => {
    const result = parsePastePrices(
      [
        "# 注释行",
        "",
        "5_30853 1金20银",
        "云芝\t120金",
        "5_89167 1砖2金",
      ].join("\n"),
      wupin,
    );
    expect(result.errors).toEqual([]);
    expect(result.entries).toEqual([
      { wupinId: "5_30853", danjia: 12_000, line: 3 },
      { wupinId: "5_30845", danjia: 1_200_000, line: 4 },
      { wupinId: "5_89167", danjia: 100_020_000, line: 5 },
    ]);
  });

  it("逐行报告错误：格式/未知物品/非法价格/非正价格", () => {
    const result = parsePastePrices(
      [
        "没有价格的一行",
        "999_999 100",
        "5_30853 不是价格",
        "5_30853 0",
        "5_30853 1金",
      ].join("\n"),
      wupin,
    );
    expect(result.entries).toHaveLength(1);
    expect(result.entries[0].wupinId).toBe("5_30853");
    expect(result.errors).toHaveLength(4);
    expect(result.errors[0]).toContain("格式");
    expect(result.errors[1]).toContain("找不到物品");
    expect(result.errors[2]).toContain("无法解析");
    expect(result.errors[3]).toContain("必须大于 0");
  });

  it("支持全角逗号分隔", () => {
    const result = parsePastePrices("5_30845，1金", wupin);
    expect(result.entries).toEqual([{ wupinId: "5_30845", danjia: 10_000, line: 1 }]);
  });
});
