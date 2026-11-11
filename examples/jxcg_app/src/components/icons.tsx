import type { SVGProps } from "react";

// 自绘 SVG 线条图标库（薄荷/松石绿主题，stroke=currentColor 继承 CSS 着色）
// 统一 viewBox 24，line 图标，圆角线帽；不依赖第三方图标库（沿用 jpcg 风格）。

interface IconProps extends SVGProps<SVGSVGElement> {
  size?: number;
}

function base(props: IconProps, size?: number) {
  const { size: s = size ?? 18, ...rest } = props;
  return {
    width: s,
    height: s,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.8,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
    ...rest,
  };
}

/** 计算 / 表格+折线 */
export function IconCalc(props: IconProps) {
  return (
    <svg {...base(props)}>
      <rect x="4" y="3.5" width="16" height="17" rx="2.5" />
      <path d="M8 8.5h8M8 12h8M8 15.5h4" />
    </svg>
  );
}

/** 行情 / 钱袋金币 */
export function IconCoin(props: IconProps) {
  return (
    <svg {...base(props)}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5v9M9 10.2h6M9 13.8h6" />
    </svg>
  );
}

/** 排行 / 榜单柱状 */
export function IconRank(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M4 20V10.5M9.3 20V5M14.7 20v-7.5M20 20V8" />
    </svg>
  );
}

/** 设置 / 齿轮 */
export function IconGear(props: IconProps) {
  return (
    <svg {...base(props)}>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5 5l1.6 1.6M17.4 17.4 19 19M19 5l-1.6 1.6M6.6 17.4 5 19" />
    </svg>
  );
}

/** 展开/折叠 左箭头 */
export function IconChevronLeft(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M14.5 6 9 12l5.5 6" />
    </svg>
  );
}

/** 展开/折叠 右箭头 */
export function IconChevronRight(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M9.5 6 15 12l-5.5 6" />
    </svg>
  );
}

/** 主题：太阳 */
export function IconSun(props: IconProps) {
  return (
    <svg {...base(props)}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5 5l1.4 1.4M17.6 17.6 19 19M19 5l-1.4 1.4M6.4 17.6 5 19" />
    </svg>
  );
}

/** 主题：月亮 */
export function IconMoon(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z" />
    </svg>
  );
}

/** 收藏 / 星标 */
export function IconStar(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M12 3.5 14.6 9l6 .6-4.5 4 .1 6-4.2-2.5L7.8 19.6l.1-6-4.5-4 6-.6z" />
    </svg>
  );
}

/** 刷新 / 循环箭头 */
export function IconRefresh(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M20 11.5A8 8 0 0 0 6.2 6.2L4 8.5" />
      <path d="M4 4.5v4h4" />
      <path d="M4 12.5a8 8 0 0 0 13.8 5.3L20 15.5" />
      <path d="M20 19.5v-4h-4" />
    </svg>
  );
}

/** 导出 / 下载 */
export function IconDownload(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M12 3.5v11M7.5 10l4.5 4.5L16.5 10" />
      <path d="M4.5 17v2.5a1 1 0 0 0 1 1h13a1 1 0 0 0 1-1V17" />
    </svg>
  );
}

/** 导入 / 上传 */
export function IconUpload(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M12 14.5v-11M7.5 8 12 3.5 16.5 8" />
      <path d="M4.5 17v2.5a1 1 0 0 0 1 1h13a1 1 0 0 0 1-1V17" />
    </svg>
  );
}

/** 时间 / 时钟 */
export function IconClock(props: IconProps) {
  return (
    <svg {...base(props)}>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5V12l3 2" />
    </svg>
  );
}

/** 搜索 / 放大镜 */
export function IconSearch(props: IconProps) {
  return (
    <svg {...base(props)}>
      <circle cx="10.5" cy="10.5" r="6.5" />
      <path d="M15.5 15.5 20 20" />
    </svg>
  );
}

/** 快照 / 相机 */
export function IconCamera(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M4 8.5h3l1.5-2h7L17 8.5h3a1 1 0 0 1 1 1V19a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V9.5a1 1 0 0 1 1-1z" />
      <circle cx="12" cy="14" r="3.5" />
    </svg>
  );
}

/** 反馈 / 虫子 */
export function IconBug(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M9 4 7.5 2.5M15 4l1.5-1.5" />
      <rect x="8" y="6" width="8" height="13" rx="3" />
      <path d="M8 10H4.5M15.5 10H19.5M8 14.5H4.5M15.5 14.5H19.5M9 7 7.5 5.5M15 7l1.5-1.5" />
    </svg>
  );
}

/** 关闭 / X */
export function IconClose(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M6 6l12 12M18 6 6 18" />
    </svg>
  );
}

/** Toast 成功 / 对勾 */
export function IconCheck(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M5 12.5l4.5 4.5L19 7" />
    </svg>
  );
}

/** Toast 警告 / 感叹号 */
export function IconAlert(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M12 3.5 21 20H3z" />
      <path d="M12 10v4M12 17.5v.1" />
    </svg>
  );
}

/** 趋势 / 折线上升 */
export function IconTrend(props: IconProps) {
  return (
    <svg {...base(props)}>
      <path d="M3.5 17.5 10 11l4 4 6.5-7.5" />
      <path d="M14.5 7.5h6v6" />
    </svg>
  );
}
