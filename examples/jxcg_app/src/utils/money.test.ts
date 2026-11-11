import { describe, expect, it } from "vitest";
import { formatTong, formatTongCompact, jinToTong, parseJin } from "./money";

/** 断言解析成功并返回铜值。 */
function ok(text: string): number {
  const result = parseJin(text);
  expect(result.ok, `应能解析「${text}」`).toBe(true);
  return result.ok ? result.tong : NaN;
}

/** 断言解析失败并返回中文错误。 */
function err(text: string): string {
  const result = parseJin(text);
  expect(result.ok, `应拒绝「${text}」`).toBe(false);
  return result.ok ? "" : result.error;
}

describe("金额解析（对齐 jxcg_core::money::parse_jin）", () => {
  it("单位解析与顺序", () => {
    expect(ok("0")).toBe(0);
    expect(ok("0铜")).toBe(0);
    expect(ok("1铜")).toBe(1);
    expect(ok("1银")).toBe(100);
    expect(ok("1金")).toBe(10_000);
    expect(ok("1砖")).toBe(100_000_000);
    expect(ok("1砖2金")).toBe(100_020_000);
    expect(ok("1金20银3铜")).toBe(12_003);
    expect(ok("3铜2银1金")).toBe(10_203);
    expect(ok(" 1,234 金 ")).toBe(12_340_000);
    expect(ok("1，234金")).toBe(12_340_000);
    expect(ok("1 砖 0 金 0 银 1 铜")).toBe(100_000_001);
    expect(ok("0砖1金")).toBe(10_000);
  });

  it("小数与纯数字按金", () => {
    expect(ok("123.45")).toBe(1_234_500);
    expect(ok("123.4567")).toBe(1_234_567);
    expect(ok("0.01")).toBe(100);
    expect(ok("0.0001")).toBe(1);
    expect(ok("12345")).toBe(123_450_000);
    expect(ok(".5")).toBe(5_000);
    expect(ok("1.5金")).toBe(15_000);
    expect(ok("1.05金")).toBe(10_500);
    expect(ok("1.5银")).toBe(150);
    expect(ok("0.05金")).toBe(500);
    expect(ok("1.0500金")).toBe(10_500);
  });

  it("负数与符号校验", () => {
    expect(ok("-1金")).toBe(-10_000);
    expect(ok("负1金20银3铜")).toBe(-12_003);
    expect(ok("-1砖")).toBe(-100_000_000);
    expect(ok("-0金")).toBe(0);
    expect(ok("负0铜")).toBe(0);
    for (const bad of ["-", "负", "--1金", "负-1金", "+-1金", "1金-2银", "负负1金"]) {
      err(bad);
    }
  });

  it("精度错误", () => {
    expect(err("3.5铜")).toContain("精度");
    expect(err("0.001银")).toContain("精度");
    expect(err("1.5铜")).toContain("精度");
    expect(err("0.000000001砖")).toContain("精度");
    expect(ok("0.0001金")).toBe(1);
    expect(ok("0.01银")).toBe(1);
    expect(ok("0.00000001砖")).toBe(1);
  });

  it("非法输入给中文提示", () => {
    expect(err("")).toBe("金额为空");
    expect(err("   ")).toBe("金额为空");
    expect(err("金")).toContain("缺少数字");
    expect(err("1金2")).toContain("缺少单位");
    expect(err("1g")).toContain("非法数字");
    expect(err("abc")).toContain("非法数字");
    expect(err("1.2.3")).toContain("非法数字");
    expect(err("1金.5")).toContain("缺少单位");
  });

  it("超范围安全拒绝", () => {
    // JS number 只能精确到 2^53，超出安全整数范围给出中文错误（Rust 侧 i64 范围更宽）。
    expect(ok("9007199254740991铜")).toBe(9_007_199_254_740_991);
    expect(err("9223372036854775808铜")).toContain("过大");
  });
});

describe("金额格式化（对齐 jxcg_core::money::format_tong）", () => {
  it("0 项省略、负数加「负」", () => {
    expect(formatTong(0)).toBe("0铜");
    expect(formatTong(1)).toBe("1铜");
    expect(formatTong(99)).toBe("99铜");
    expect(formatTong(100)).toBe("1银");
    expect(formatTong(101)).toBe("1银1铜");
    expect(formatTong(10_000)).toBe("1金");
    expect(formatTong(12_003)).toBe("1金20银3铜");
    expect(formatTong(100_000_000)).toBe("1砖");
    expect(formatTong(100_020_003)).toBe("1砖2金3铜");
    expect(formatTong(-1)).toBe("负1铜");
    expect(formatTong(-12_003)).toBe("负1金20银3铜");
  });

  it("格式化后可解析回原值", () => {
    for (const value of [0, 1, 99, 100, 101, 9_999, 10_000, 12_345, 99_999_999, 100_000_000, 100_020_003, -1, -12_003]) {
      expect(ok(formatTong(value))).toBe(value);
    }
  });

  it("紧凑格式用于表格/图表", () => {
    expect(formatTongCompact(0)).toBe("0铜");
    expect(formatTongCompact(5_000)).toBe("50银");
    expect(formatTongCompact(12_000)).toBe("1.2金");
    expect(formatTongCompact(100_000_000)).toBe("1砖");
    expect(jinToTong(1.2345)).toBe(12_345);
  });
});
