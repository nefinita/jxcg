import { useCallback, useEffect, useState } from "react";
import { STORAGE_KEYS } from "../utils/constants";

export type Theme = "dark" | "light";

/** 主题：默认浅色，localStorage 记住选择，写到 <html data-theme>。 */
export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    const stored = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEYS.theme) : null;
    return stored === "dark" ? "dark" : "light";
  });

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    try {
      localStorage.setItem(STORAGE_KEYS.theme, theme);
    } catch {
      /* 存储不可用时仅当前会话生效。 */
    }
  }, [theme]);

  const toggleTheme = useCallback(() => {
    setThemeState((prev) => (prev === "dark" ? "light" : "dark"));
  }, []);

  const setTheme = useCallback((next: Theme) => setThemeState(next), []);

  return { theme, toggleTheme, setTheme };
}
