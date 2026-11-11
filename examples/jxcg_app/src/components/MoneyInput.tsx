import { useEffect, useRef, useState } from "react";
import type { Tong } from "../types";
import clsx from "../utils/clsx";
import { formatTong, parseJin } from "../utils/money";
import styles from "./MoneyInput.module.css";

export interface MoneyInputProps {
  /** 当前金额（铜） */
  value: Tong;
  onChange: (tong: Tong) => void;
  id?: string;
  /** 无可见 label 时的无障碍名称 */
  ariaLabel?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 是否允许负数（默认 false；利润展示用 ResultCard，不需要输入负数） */
  allowNegative?: boolean;
  /** 非法输入回调（null 表示恢复合法） */
  onErrorChange?: (error: string | null) => void;
  className?: string;
  /** compact：表格/行内改价用的小尺寸 */
  size?: "normal" | "compact";
  /** 输入框变化时回调原始文本（可选，便于外部联动） */
  onTextChange?: (text: string) => void;
}

/**
 * 金额输入：支持 `1金20银3铜`、`123.4567`（金）、`1砖` 等格式（见 utils/money.ts）。
 * 聚焦时显示可编辑文本，失焦后显示规范格式；非法输入不提交，只提示错误。
 */
export default function MoneyInput({
  value,
  onChange,
  id,
  ariaLabel,
  placeholder = "如 1金20银3铜 / 123.45 / 1砖",
  disabled = false,
  allowNegative = false,
  onErrorChange,
  className,
  size = "normal",
  onTextChange,
}: MoneyInputProps) {
  const [focused, setFocused] = useState(false);
  const [text, setText] = useState(() => formatTong(value));
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const focusedRef = useRef(false);
  focusedRef.current = focused;

  useEffect(() => {
    // 非聚焦时始终跟随外部 value（例如重置表单/切换配方）。
    if (!focused) {
      setText(formatTong(value));
      setError(null);
    }
  }, [value, focused]);

  const commit = (raw: string) => {
    const parsed = parseJin(raw);
    if (!parsed.ok) {
      setError(parsed.error);
      onErrorChange?.(parsed.error);
      return false;
    }
    if (!allowNegative && parsed.tong < 0) {
      const message = "金额不能为负";
      setError(message);
      onErrorChange?.(message);
      return false;
    }
    setError(null);
    onErrorChange?.(null);
    onChange(parsed.tong);
    return true;
  };

  const handleChange = (raw: string) => {
    setText(raw);
    onTextChange?.(raw);
    commit(raw);
  };

  const displayed = focused ? text : formatTong(value);

  return (
    <span className={clsx(styles.wrap, size === "compact" && styles.compact, className)}>
      <input
        ref={inputRef}
        id={id}
        className={clsx(styles.input, error && styles.inputInvalid)}
        type="text"
        inputMode="text"
        autoComplete="off"
        spellCheck={false}
        value={displayed}
        placeholder={placeholder}
        disabled={disabled}
        aria-label={ariaLabel}
        aria-invalid={error !== null}
        onFocus={(event) => {
          setFocused(true);
          setText(formatTong(value));
          event.currentTarget.select();
        }}
        onBlur={() => {
          setFocused(false);
          setError(null);
          onErrorChange?.(null);
        }}
        onChange={(event) => handleChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit(event.currentTarget.value);
            event.currentTarget.blur();
          }
        }}
      />
      <span className={error ? styles.error : styles.hint} aria-live="polite">
        {error ?? (focused ? `= ${formatTong(value)}` : "")}
      </span>
    </span>
  );
}
