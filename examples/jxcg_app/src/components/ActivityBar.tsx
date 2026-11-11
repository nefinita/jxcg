import { useState } from "react";
import clsx from "../utils/clsx";
import {
  IconCalc,
  IconChevronLeft,
  IconChevronRight,
  IconCoin,
  IconGear,
  IconRank,
} from "./icons";
import styles from "./ActivityBar.module.css";

export type Page = "calc" | "market" | "rank" | "settings";

interface Props {
  currentPage: Page;
  onNavigate: (page: Page) => void;
}

const ITEMS: ReadonlyArray<{ page: Page; Icon: typeof IconCalc; label: string }> = [
  { page: "calc", Icon: IconCalc, label: "计算" },
  { page: "market", Icon: IconCoin, label: "行情" },
  { page: "rank", Icon: IconRank, label: "排行" },
  { page: "settings", Icon: IconGear, label: "设置" },
];

/** 左侧图标导航栏：可展开显示文字（对齐 jpcg ActivityBar）。 */
export default function ActivityBar({ currentPage, onNavigate }: Props) {
  const [expanded, setExpanded] = useState(false);

  return (
    <nav className={clsx(styles.bar, expanded && styles.expanded)} aria-label="主导航">
      {ITEMS.map(({ page, Icon, label }) => (
        <button
          key={page}
          className={clsx(styles.btn, currentPage === page && styles.active)}
          onClick={() => onNavigate(page)}
          title={label}
          aria-current={currentPage === page ? "page" : undefined}
        >
          <span className={styles.icon}>
            <Icon size={18} />
          </span>
          {expanded && <span className={styles.label}>{label}</span>}
        </button>
      ))}
      {expanded && <div className={styles.spacer} />}
      <button
        className={styles.toggleBtn}
        onClick={() => setExpanded((v) => !v)}
        title={expanded ? "收起导航" : "展开导航"}
        aria-label={expanded ? "收起导航" : "展开导航"}
      >
        {expanded ? <IconChevronLeft size={14} /> : <IconChevronRight size={14} />}
      </button>
    </nav>
  );
}
