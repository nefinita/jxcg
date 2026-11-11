// ============================================================================
// localStorage 持久化：版本信封 + 校验，读写失败不影响页面继续使用内存状态。
// 对齐 jpcg 的 hooks/usePersistentState.ts，schema 校验放在 state/schemas.ts。
// ============================================================================

import { useCallback, useEffect, useState, type SetStateAction } from "react";
import type { PersistentSchema } from "../state/schemas";

export function readPreference(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function writePreference(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    /* 无痕模式/存储已满时忽略，页面仍可用内存状态。 */
  }
}

/** 读取并校验一条持久化状态；任何异常/版本不符/校验失败都回退 fallback。 */
export function readPersistentState<T>(
  key: string,
  fallback: () => T,
  schema: PersistentSchema<T>,
): T {
  try {
    const raw: unknown = JSON.parse(readPreference(key) ?? "null");
    if (raw === null) return fallback();
    const envelope =
      typeof raw === "object" && raw !== null && "version" in raw && "data" in raw
        ? (raw as { version: number; data: unknown })
        : { version: 0, data: raw };
    // 未知的未来版本不可降级解释。
    if (!Number.isFinite(envelope.version) || envelope.version > schema.version) return fallback();
    const value =
      envelope.version === schema.version ? envelope.data : schema.migrate?.(envelope.data, envelope.version);
    return schema.validate(value) ? value : fallback();
  } catch {
    return fallback();
  }
}

/** 持久化状态 hook：setValue 后异步写入 localStorage（仅用户改动才写）。 */
export function usePersistentState<T>(key: string, fallback: () => T, schema: PersistentSchema<T>) {
  const [entry, setEntry] = useState(() => ({
    key,
    value: readPersistentState(key, fallback, schema),
    dirty: false,
  }));
  let current = entry;
  if (entry.key !== key) {
    current = { key, value: readPersistentState(key, fallback, schema), dirty: false };
    setEntry(current);
  }
  useEffect(() => {
    if (!current.dirty) return;
    writePreference(current.key, JSON.stringify({ version: schema.version, data: current.value }));
  }, [current, schema.version]);
  const setValue = useCallback(
    (update: SetStateAction<T>) => {
      setEntry((previous) =>
        previous.key !== key
          ? previous
          : {
              key,
              dirty: true,
              value:
                typeof update === "function"
                  ? (update as (value: T) => T)(previous.value)
                  : update,
            },
      );
    },
    [key],
  );
  return [current.value, setValue] as const;
}
