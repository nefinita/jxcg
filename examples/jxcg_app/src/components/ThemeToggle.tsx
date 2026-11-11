import type { Theme } from "../hooks/useTheme";
import { IconMoon, IconSun } from "./icons";
import styles from "./ThemeToggle.module.css";

interface Props {
  theme: Theme;
  onToggle: () => void;
}

/** 深浅色主题切换按钮（沿用 jpcg 样式）。 */
export default function ThemeToggle({ theme, onToggle }: Props) {
  return (
    <button
      className={styles.btn}
      onClick={onToggle}
      title={theme === "dark" ? "切换到浅色" : "切换到深色"}
      aria-label="切换主题"
    >
      {theme === "dark" ? <IconSun size={16} /> : <IconMoon size={16} />}
    </button>
  );
}
