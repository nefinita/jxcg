import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { readPersistentState, usePersistentState } from "./usePersistentState";
import type { PersistentSchema } from "../state/schemas";

interface Box {
  n: number;
  note: string;
}

const boxSchema: PersistentSchema<Box> = {
  version: 2,
  validate: (value): value is Box =>
    typeof value === "object" &&
    value !== null &&
    typeof (value as Box).n === "number" &&
    typeof (value as Box).note === "string",
  migrate: (value, version) => {
    if (version === 1 && typeof value === "object" && value !== null && typeof (value as { n?: unknown }).n === "number") {
      return { n: (value as { n: number }).n, note: "已迁移" };
    }
    return value;
  },
};

const KEY = "jxcg_test_state";

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  localStorage.clear();
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
});

function PersistedHarness({ onValue }: { onValue?: (value: Box) => void }) {
  const [value, setValue] = usePersistentState<Box>(KEY, () => ({ n: 0, note: "默认" }), boxSchema);
  onValue?.(value);
  return (
    <button onClick={() => setValue((previous) => ({ ...previous, n: previous.n + 1 }))}>
      {value.n}:{value.note}
    </button>
  );
}

async function click() {
  await act(async () => host.querySelector("button")!.dispatchEvent(new MouseEvent("click", { bubbles: true })));
}

describe("持久化状态（版本信封 + schema 校验）", () => {
  it("无数据时使用 fallback", () => {
    expect(readPersistentState(KEY, () => ({ n: 0, note: "默认" }), boxSchema)).toEqual({ n: 0, note: "默认" });
  });

  it("读取合法信封数据", () => {
    localStorage.setItem(KEY, JSON.stringify({ version: 2, data: { n: 7, note: "ok" } }));
    expect(readPersistentState(KEY, () => ({ n: 0, note: "默认" }), boxSchema)).toEqual({ n: 7, note: "ok" });
  });

  it("损坏 JSON / 校验失败 / 未来版本都安全回退", () => {
    localStorage.setItem(KEY, "{not json");
    expect(readPersistentState(KEY, () => ({ n: 1, note: "默认" }), boxSchema).n).toBe(1);
    localStorage.setItem(KEY, JSON.stringify({ version: 2, data: { n: "坏", note: 1 } }));
    expect(readPersistentState(KEY, () => ({ n: 2, note: "默认" }), boxSchema).n).toBe(2);
    localStorage.setItem(KEY, JSON.stringify({ version: 99, data: { n: 3, note: "future" } }));
    expect(readPersistentState(KEY, () => ({ n: 4, note: "默认" }), boxSchema).n).toBe(4);
  });

  it("旧版本走 migrate", () => {
    localStorage.setItem(KEY, JSON.stringify({ version: 1, data: { n: 5 } }));
    expect(readPersistentState(KEY, () => ({ n: 0, note: "默认" }), boxSchema)).toEqual({ n: 5, note: "已迁移" });
  });

  it("hook 变更写入 localStorage，重新挂载后恢复", async () => {
    await act(async () => root.render(<PersistedHarness />));
    await click();
    const raw = JSON.parse(localStorage.getItem(KEY)!);
    expect(raw).toEqual({ version: 2, data: { n: 1, note: "默认" } });

    await act(async () => root.unmount());
    root = createRoot(host);
    await act(async () => root.render(<PersistedHarness />));
    expect(host.textContent).toContain("1:默认");
  });
});
