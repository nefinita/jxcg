// ============================================================================
// 数字 / 百分比 / 时间格式化
// ============================================================================

/** 大数展示（对齐 jpcg 的万/亿口径）。 */
export function formatNumber(n: number): string {
  if (!Number.isFinite(n)) return "-";
  const abs = Math.abs(n);
  if (abs >= 100_000_000) return `${(n / 100_000_000).toFixed(2)}亿`;
  if (abs >= 10_000) return `${(n / 10_000).toFixed(1)}万`;
  return n.toLocaleString("zh-CN");
}

/** 万分比 → 百分比字符串（500 → `5%`，4750 → `47.5%`）。 */
export function formatBp(bp: number): string {
  if (!Number.isFinite(bp)) return "-";
  const pct = bp / 100;
  const text = pct.toFixed(2).replace(/\.?0+$/, "");
  return `${text}%`;
}

/** 0~1 的比例 → 百分比字符串。 */
export function formatPercent(value: number): string {
  if (!Number.isFinite(value)) return "-";
  return `${(value * 100).toFixed(1)}%`;
}

/** 期望件数等小数：最多 2 位、去掉尾随 0。 */
export function formatQiwang(n: number): string {
  if (!Number.isFinite(n)) return "-";
  return n.toFixed(2).replace(/\.?0+$/, "");
}

/** 秒级时间戳 → `YYYY-MM-DD HH:mm`；0/空显示「—」。 */
export function formatShijian(seconds: number): string {
  if (!seconds || !Number.isFinite(seconds)) return "—";
  const d = new Date(seconds * 1000);
  const pad = (v: number) => String(v).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 秒级时间戳 → `MM-DD HH:mm`（图表轴/紧凑列表用）。 */
export function formatShijianShort(seconds: number): string {
  if (!seconds || !Number.isFinite(seconds)) return "—";
  const d = new Date(seconds * 1000);
  const pad = (v: number) => String(v).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 当前时间戳（秒）。 */
export function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

/** 把任意输入夹到 [min, max] 的整数（输入框用）。 */
export function clampInt(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, Math.trunc(value)));
}
