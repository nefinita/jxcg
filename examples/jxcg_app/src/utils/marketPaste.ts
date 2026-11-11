// ============================================================================
// 行情「粘贴批量导入」解析：每行 `物品ID或名称 价格`
//
// 支持价格格式见 utils/money.ts：`1金20银3铜`、`123.45`（金）、`1砖`；
// 以 # 开头的行视为注释；分隔符支持空格/制表符/半角逗号/全角逗号。
// ============================================================================

import type { Tong, Wupin } from "../types";
import { parseJin } from "./money";

export interface PasteEntry {
  wupinId: string;
  danjia: Tong;
  /** 原行号（从 1 开始，便于错误提示） */
  line: number;
}

export interface PasteResult {
  entries: PasteEntry[];
  errors: string[];
}

/** 解析批量粘贴文本；返回成功条目与逐行错误（中文）。 */
export function parsePastePrices(text: string, wupinList: Wupin[]): PasteResult {
  const entries: PasteEntry[] = [];
  const errors: string[] = [];
  const byId = new Map<string, Wupin>();
  const byName = new Map<string, Wupin>();
  for (const wupin of wupinList) {
    byId.set(wupin.id, wupin);
    if (!byName.has(wupin.mingcheng)) byName.set(wupin.mingcheng, wupin);
  }

  const lines = text.split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index];
    const lineNumber = index + 1;
    const line = rawLine.replace(/#.*$/, "").trim();
    if (line === "") continue;

    const matched = /^(\S+)[\s,，\t]+(.+)$/.exec(line);
    if (!matched) {
      errors.push(`第 ${lineNumber} 行：格式应为「物品ID或名称 价格」`);
      continue;
    }
    const key = matched[1];
    const priceText = matched[2].replace(/\s+/g, "");
    const wupin = byId.get(key) ?? byName.get(key);
    if (!wupin) {
      errors.push(`第 ${lineNumber} 行：找不到物品「${key}」`);
      continue;
    }
    const parsed = parseJin(priceText);
    if (!parsed.ok) {
      errors.push(`第 ${lineNumber} 行：价格「${matched[2]}」无法解析（${parsed.error}）`);
      continue;
    }
    if (parsed.tong <= 0) {
      errors.push(`第 ${lineNumber} 行：价格必须大于 0`);
      continue;
    }
    entries.push({ wupinId: wupin.id, danjia: parsed.tong, line: lineNumber });
  }

  return { entries, errors };
}
