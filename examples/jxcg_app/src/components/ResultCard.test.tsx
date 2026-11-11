import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import ResultCard, { formatResultValue, type ResultItem } from "./ResultCard";

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

const items: ResultItem[] = [
  { label: "总成本", value: 12_003, kind: "money" },
  { label: "净利润", value: -2_000, kind: "money", tone: "negative" },
  { label: "利润率", value: 4_750, kind: "bp" },
  { label: "期望产出", value: 2.5, kind: "number" },
  { label: "备注", value: "缺 1 个材料价" },
];

async function render(data: ResultItem[], title?: string) {
  await act(async () => root.render(<ResultCard title={title} items={data} />));
}

describe("结果卡", () => {
  it("按类型格式化金额/万分比/数字/文本", async () => {
    await render(items, "云芝竹沥羹");
    expect(host.textContent).toContain("云芝竹沥羹");
    expect(host.textContent).toContain("1金20银3铜");
    expect(host.textContent).toContain("负20银");
    expect(host.textContent).toContain("47.5%");
    expect(host.textContent).toContain("2.5");
    expect(host.textContent).toContain("缺 1 个材料价");
  });

  it("负值/负调性使用 negative 语义样式", async () => {
    await render(items);
    const negative = host.querySelector('[data-tone="negative"]');
    expect(negative).not.toBeNull();
    expect(negative!.querySelector("dd")!.className).toContain("negative");
  });

  it("空数据展示占位文案", async () => {
    await render([], "结果");
    expect(host.textContent).toContain("暂无结果");
  });

  it("formatResultValue 对异常数字返回占位", () => {
    expect(formatResultValue({ value: Number.NaN, kind: "money" })).toBe("—");
    expect(formatResultValue({ value: 10_000, kind: "money" })).toBe("1金");
    expect(formatResultValue({ value: 0.425, kind: "percent" })).toBe("42.5%");
  });
});
