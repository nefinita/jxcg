import type { ReactNode } from "react";
import clsx from "../utils/clsx";
import { formatBp, formatPercent, formatQiwang } from "../utils/format";
import { formatTong } from "../utils/money";
import styles from "./ResultCard.module.css";

/** 结果项类型：money=铜、bp=万分比、percent=0~1 比例、number=普通数字、text=原文。 */
export type ResultKind = "money" | "bp" | "percent" | "number" | "text";

export type ResultTone = "default" | "positive" | "negative" | "muted";

export interface ResultItem {
  label: string;
  value: number | string;
  kind?: ResultKind;
  tone?: ResultTone;
  /** 补充说明（小字） */
  hint?: string;
  /** 主要指标：加大字号 */
  emphasis?: boolean;
}

export interface ResultCardProps {
  title?: string;
  items: ResultItem[];
  footer?: ReactNode;
  /** 无数据时的占位（默认按条数显示） */
  emptyText?: string;
  className?: string;
  /** 卡片右侧附加内容（例如「缺价」提示） */
  extra?: ReactNode;
}

/** 按 kind 格式化结果值（导出便于测试与复用）。 */
export function formatResultValue(item: Pick<ResultItem, "value" | "kind">): string {
  const { value, kind = "text" } = item;
  if (kind === "text") return String(value);
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return "—";
  switch (kind) {
    case "money":
      return formatTong(numeric);
    case "bp":
      return formatBp(numeric);
    case "percent":
      return formatPercent(numeric);
    case "number":
      return Number.isInteger(numeric) ? numeric.toLocaleString("zh-CN") : formatQiwang(numeric);
    default:
      return String(value);
  }
}

/** 结果卡：一组指标的网格展示（正/负/强调语义色）。 */
export default function ResultCard({
  title,
  items,
  footer,
  emptyText = "暂无结果",
  className,
  extra,
}: ResultCardProps) {
  if (items.length === 0) {
    return (
      <section className={clsx(styles.card, className)}>
        {title && <h2 className={styles.title}>{title}</h2>}
        <p className={styles.empty}>{emptyText}</p>
      </section>
    );
  }
  return (
    <section className={clsx(styles.card, className)}>
      {(title || extra) && (
        <header className={styles.header}>
          {title && <h2 className={styles.title}>{title}</h2>}
          {extra}
        </header>
      )}
      <dl className={styles.grid}>
        {items.map((item) => {
          const numeric = typeof item.value === "number" ? item.value : Number(item.value);
          const autoTone: ResultTone =
            item.tone ?? (Number.isFinite(numeric) && numeric < 0 && item.kind !== "text" ? "negative" : "default");
          return (
            <div
              key={item.label}
              className={clsx(styles.item, item.emphasis && styles.emphasis)}
              data-tone={autoTone}
            >
              <dt className={styles.label}>{item.label}</dt>
              <dd className={clsx(styles.value, autoTone !== "default" && styles[autoTone])}>
                {formatResultValue(item)}
              </dd>
              {item.hint && <span className={styles.sub}>{item.hint}</span>}
            </div>
          );
        })}
      </dl>
      {footer && <div className={styles.footer}>{footer}</div>}
    </section>
  );
}
