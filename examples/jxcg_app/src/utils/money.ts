// ============================================================================
// 金 / 银 / 铜 解析与格式化（镜像 crates/jxcg_core/src/money.rs）
//
// - 1 金 = 100 银 = 10_000 铜；1 砖 = 10_000 金 = 100_000_000 铜；
// - 内部整数铜运算（number），解析结果可以为负（用于展示负利润）；
// - 输入兼容「123金45银67铜」「123.4567金」「123.45」「12345」「-1金」「负1金20银」；
// - 输出固定为「1砖2金3银4铜」，0 项省略、负数加「负」前缀；
// - 精度规则：砖最多 8 位小数、金 4 位、银 2 位、铜 0 位；纯数字按金解析。
// ============================================================================

import type { Tong } from "../types";

const TONG_PER_YIN = 100;
const TONG_PER_JIN = 10_000;
const TONG_PER_ZHUAN = 100_000_000;

/** 解析结果：成功给出铜值，失败给出中文错误提示。 */
export type MoneyParseResult = { ok: true; tong: Tong } | { ok: false; error: string };

function unitOf(ch: string): number | null {
  switch (ch) {
    case "砖":
      return TONG_PER_ZHUAN;
    case "金":
      return TONG_PER_JIN;
    case "银":
      return TONG_PER_YIN;
    case "铜":
      return 1;
    default:
      return null;
  }
}

function isAsciiDigit(ch: string): boolean {
  return ch >= "0" && ch <= "9";
}

function fracLimit(unit: number): number {
  if (unit === TONG_PER_ZHUAN) return 8;
  if (unit === TONG_PER_JIN) return 4;
  if (unit === TONG_PER_YIN) return 2;
  return 0;
}

/** 按单位解析一个数字段（可带小数），返回正数铜值。 */
function parseAmount(
  text: string,
  unit: number,
): { ok: true; value: number } | { ok: false; error: string } {
  if (text === "") return { ok: false, error: "金额为空" };
  const dots = text.split(".").length - 1;
  if (dots > 1) return { ok: false, error: `非法数字「${text}」` };
  const dotIndex = text.indexOf(".");
  const intPart = dotIndex < 0 ? text : text.slice(0, dotIndex);
  const fracPart = dotIndex < 0 ? "" : text.slice(dotIndex + 1);
  if (intPart === "" && fracPart === "") return { ok: false, error: "金额为空" };
  if (!Array.from(intPart).every(isAsciiDigit) || !Array.from(fracPart).every(isAsciiDigit)) {
    return { ok: false, error: `非法数字「${text}」` };
  }
  const maxFrac = fracLimit(unit);
  if (fracPart.length > maxFrac) {
    return { ok: false, error: `小数位数超出精度（该单位最多 ${maxFrac} 位）` };
  }
  const intValue = intPart === "" ? 0 : Number(intPart);
  if (!Number.isSafeInteger(intValue)) return { ok: false, error: `非法或过大数字「${text}」` };
  let value = intValue * unit;
  if (!Number.isSafeInteger(value)) return { ok: false, error: "金额超出范围" };
  if (fracPart !== "") {
    const fracValue = Number(fracPart);
    const scale = Math.pow(10, fracPart.length);
    const fracTong = (fracValue * unit) / scale;
    if (!Number.isInteger(fracTong)) {
      return { ok: false, error: `金额精度超过 1 铜「${text}」` };
    }
    value += fracTong;
    if (!Number.isSafeInteger(value)) return { ok: false, error: "金额超出范围" };
  }
  return { ok: true, value };
}

/** 解析金额文本，返回铜（可带负号）。 */
export function parseJin(input: string): MoneyParseResult {
  const s = Array.from(input)
    .filter((ch) => !/\s/.test(ch) && ch !== "," && ch !== "，")
    .join("");
  if (s === "") return { ok: false, error: "金额为空" };

  let rest = s;
  let negative = false;
  if (rest.startsWith("负")) {
    negative = true;
    rest = rest.slice(1);
  } else if (rest.startsWith("-")) {
    negative = true;
    rest = rest.slice(1);
  }
  if (rest === "") return { ok: false, error: "金额为空" };
  if (rest.startsWith("负") || rest.startsWith("-") || rest.startsWith("+")) {
    return { ok: false, error: "金额符号重复或非法" };
  }

  const hasUnit = Array.from(rest).some((ch) => unitOf(ch) !== null);
  let magnitude = 0;
  if (hasUnit) {
    let num = "";
    let sawUnit = false;
    for (const ch of Array.from(rest)) {
      const unit = unitOf(ch);
      if (unit !== null) {
        if (num === "") return { ok: false, error: `单位「${ch}」前缺少数字` };
        const parsed = parseAmount(num, unit);
        if (!parsed.ok) return parsed;
        magnitude += parsed.value;
        if (!Number.isSafeInteger(magnitude)) return { ok: false, error: "金额超出范围" };
        num = "";
        sawUnit = true;
        continue;
      }
      if (isAsciiDigit(ch) || ch === ".") {
        num += ch;
        continue;
      }
      return { ok: false, error: `无法识别的字符「${ch}」` };
    }
    if (num !== "") return { ok: false, error: "末尾数字缺少单位（砖/金/银/铜）" };
    if (!sawUnit) return { ok: false, error: "未找到金额单位" };
  } else {
    const parsed = parseAmount(rest, TONG_PER_JIN);
    if (!parsed.ok) return parsed;
    magnitude = parsed.value;
  }

  // 负零归一为 0（与 Rust 侧 -0金 == 0 一致）。
  const tong = magnitude === 0 ? 0 : negative ? -magnitude : magnitude;
  if (!Number.isSafeInteger(tong)) return { ok: false, error: "金额超出范围" };
  return { ok: true, tong };
}

/** 解析失败返回 null 的便捷版本（用于批量粘贴导入等场景）。 */
export function parseJinOrNull(input: string): Tong | null {
  const parsed = parseJin(input);
  return parsed.ok ? parsed.tong : null;
}

const FORMAT_UNITS: ReadonlyArray<readonly [number, string]> = [
  [TONG_PER_ZHUAN, "砖"],
  [TONG_PER_JIN, "金"],
  [TONG_PER_YIN, "银"],
  [1, "铜"],
];

/** 格式化铜为「1砖2金3银4铜」（0 项省略；负数加「负」前缀）。 */
export function formatTong(tong: Tong): string {
  if (!Number.isFinite(tong)) return "-";
  const v = Math.trunc(tong);
  if (v === 0) return "0铜";
  const negative = v < 0;
  let rest = Math.abs(v);
  const parts: string[] = [];
  for (const [unit, name] of FORMAT_UNITS) {
    const count = Math.floor(rest / unit);
    rest %= unit;
    if (count > 0) parts.push(`${count}${name}`);
  }
  if (parts.length === 0) parts.push("0铜");
  const body = parts.join("");
  return negative ? `负${body}` : body;
}

/** 表格/坐标轴用的紧凑金额：≥1 砖显示砖、≥1 金显示金，否则按金/银/铜展开。 */
export function formatTongCompact(tong: Tong): string {
  if (!Number.isFinite(tong)) return "-";
  const v = Math.trunc(tong);
  const abs = Math.abs(v);
  if (abs >= TONG_PER_ZHUAN) {
    return `${(v / TONG_PER_ZHUAN).toFixed(2).replace(/\.?0+$/, "")}砖`;
  }
  if (abs >= TONG_PER_JIN) {
    return `${(v / TONG_PER_JIN).toFixed(2).replace(/\.?0+$/, "")}金`;
  }
  return formatTong(v);
}

/** 金额全精度展示（tooltip 等场景，与 formatTong 相同）。 */
export const formatJinYinTong = formatTong;

/** 换算：金 → 铜。 */
export function jinToTong(jin: number): Tong {
  return Math.round(jin * TONG_PER_JIN);
}

/** 换算：铜 → 金（数字，用于输入框）。 */
export function tongToJin(tong: Tong): number {
  return tong / TONG_PER_JIN;
}
