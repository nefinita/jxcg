import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import CalcPage from "./CalcPage";
import { mockCatalog, mockPriceMap } from "../api/mock";
import { DEFAULT_SETTINGS, DEFAULT_SHOUXUFEI } from "../utils/constants";
import type { JiageBiao, Settings } from "../types";

function makePrices(): JiageBiao {
  const biao: JiageBiao = {};
  for (const [wupinId, danjia] of Object.entries(mockPriceMap("jiandanqinxin"))) {
    biao[`jiandanqinxin|${wupinId}`] = { danjia, gengxin_shijian: 1, laiyuan: "jx3box", shoudong: false };
  }
  return biao;
}

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

async function flush(ms = 700) {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

async function render(settings: Settings = DEFAULT_SETTINGS) {
  await act(async () =>
    root.render(
      <CalcPage
        catalog={mockCatalog()}
        settings={settings}
        updateSettings={vi.fn()}
        shouxufei={DEFAULT_SHOUXUFEI}
        prices={makePrices()}
        fuwuqiId="jiandanqinxin"
        favorites={[]}
        onToggleFavorite={vi.fn()}
        focusRecipe={null}
        addToast={vi.fn()}
        setStatus={vi.fn()}
      />,
    ),
  );
  await flush();
}

function text(): string {
  return host.textContent ?? "";
}

async function clickButton(label: string) {
  const button = [...host.querySelectorAll("button")].find((element) => element.textContent === label);
  expect(button, `找不到按钮：${label}`).toBeDefined();
  await act(async () => button!.click());
  await flush(120);
}

describe("计算页中间产物展示（task-6）", () => {
  it("默认展开中间产物：自制徽标 + 中间产物精力/摊入 + 行内明细", async () => {
    await render();
    expect(text()).toContain("材料表");
    expect(text()).toContain("自制"); // 酱料行来源徽标
    expect(text()).toContain("中间产物摊入");
    expect(text()).toContain("中间产物精力");
    expect(text()).toContain("酱料");
    // 展开中间产物明细：产出配方 + 展开的采购原料
    await clickButton("明细");
    expect(text()).toContain("产出配方：酱料制作");
    expect(text()).toContain("展开的采购原料");
    expect(text()).toContain("糯米");
    expect(text()).toContain("老豆腐");
    expect(text()).toContain("收起");
  });

  it("关闭开关时按旧行为：没有自制行/中间产物摊入", async () => {
    await render({ ...DEFAULT_SETTINGS, zhongjian_zizhi: false });
    expect(text()).toContain("材料表");
    expect(text()).not.toContain("中间产物摊入");
    expect(text()).not.toContain("中间产物精力");
    // 酱料行不再标「自制」，也没有可展开的「明细」按钮（footer 的「材料（含自制）」文案不受影响）。
    const jiangliaoRow = [...host.querySelectorAll("tbody tr")].find((row) =>
      (row.textContent ?? "").includes("酱料"),
    );
    expect(jiangliaoRow).toBeDefined();
    expect(jiangliaoRow!.textContent).not.toContain("自制");
    expect([...host.querySelectorAll("button")].some((button) => button.textContent === "明细")).toBe(false);
  });

  it("选中「只产中间产物」配方时提示自用成本，不显示可做/亏损", async () => {
    await render();
    const option = [...host.querySelectorAll<HTMLButtonElement>('[role="option"]')].find((element) =>
      (element.textContent ?? "").includes("酱料制作"),
    );
    expect(option).toBeDefined();
    await act(async () => option!.click());
    await flush();
    expect(text()).toContain("（中间产物）");
    expect(text()).toContain("中间产物仅作成本参考，默认不进入成品排行");
    expect(text()).toContain("自用成本");
  });
});
