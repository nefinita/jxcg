import { useState } from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import MoneyInput from "./MoneyInput";

/** 受控 harness：把 onChange 结果写回 value，与真实页面用法一致。 */
function Harness({
  initial,
  allowNegative = false,
  onChange,
}: {
  initial: number;
  allowNegative?: boolean;
  onChange?: (tong: number) => void;
}) {
  const [value, setValue] = useState(initial);
  return (
    <MoneyInput
      ariaLabel="金额"
      value={value}
      allowNegative={allowNegative}
      onChange={(tong) => {
        setValue(tong);
        onChange?.(tong);
      }}
    />
  );
}

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
});

function input(): HTMLInputElement {
  return host.querySelector("input")!;
}

async function render(initial: number, allowNegative = false, onChange?: (tong: number) => void) {
  await act(async () => root.render(<Harness initial={initial} allowNegative={allowNegative} onChange={onChange} />));
}

/** 模拟用户输入（React 19：需要原生 value setter + input 事件）。 */
async function type(value: string) {
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input(), value);
    input().dispatchEvent(new Event("input", { bubbles: true }));
  });
}

async function focus() {
  await act(async () => input().dispatchEvent(new FocusEvent("focusin", { bubbles: true })));
}

async function blur() {
  await act(async () => input().dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
}

describe("MoneyInput", () => {
  it("初始按规范格式展示", async () => {
    await render(12_003);
    expect(input().value).toBe("1金20银3铜");
  });

  it("聚焦后输入 2砖，解析为 2 亿铜并回调", async () => {
    const onChange = vi.fn();
    await render(12_003, false, onChange);
    await focus();
    await type("2砖");
    expect(onChange).toHaveBeenLastCalledWith(200_000_000);
    expect(input().value).toBe("2砖");
  });

  it("输入非法「1金2」不回调，并显示中文错误；失焦恢复原值", async () => {
    const onChange = vi.fn();
    await render(12_003, false, onChange);
    await focus();
    await type("1金2");
    expect(onChange).not.toHaveBeenCalled();
    expect(host.textContent).toContain("缺少单位");
    await blur();
    expect(input().value).toBe("1金20银3铜");
    expect(host.textContent).not.toContain("缺少单位");
  });

  it("默认拒绝负数，允许时接受", async () => {
    const onChange = vi.fn();
    await render(0, false, onChange);
    await focus();
    await type("-1金");
    expect(onChange).not.toHaveBeenCalled();
    expect(host.textContent).toContain("金额不能为负");
    await act(async () => root.render(<Harness initial={0} allowNegative onChange={onChange} />));
    await focus();
    await type("-1金");
    expect(onChange).toHaveBeenLastCalledWith(-10_000);
  });

  it("输入 123.4567 按金解析为 1_234_567 铜", async () => {
    const onChange = vi.fn();
    await render(0, false, onChange);
    await focus();
    await type("123.4567");
    expect(onChange).toHaveBeenLastCalledWith(1_234_567);
  });
});
