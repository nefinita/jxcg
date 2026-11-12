import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import App from "./App";

// recharts 的 ResponsiveContainer 依赖 ResizeObserver（jsdom 未实现）。
beforeAll(() => {
  class ResizeObserverMock {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  Object.assign(globalThis, { ResizeObserver: ResizeObserverMock });
});

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
});

/** 等待 mock IPC（加载目录/设置/行情、防抖计算）完成。 */
async function flush(ms = 350) {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

async function mount() {
  await act(async () => root.render(<App />));
  await flush(500);
}

async function clickButton(title: string) {
  const button = host.querySelector<HTMLButtonElement>(`button[title="${title}"]`);
  expect(button, `找不到按钮：${title}`).not.toBeNull();
  await act(async () => button!.click());
  await flush(300);
}

/** 读取当前可见页面（未被 hidden 的 residentPage）。 */
function visiblePageText(): string {
  const pages = [...host.querySelectorAll<HTMLDivElement>('[class*="residentPage"]')];
  const visible = pages.filter((element) => !element.hidden);
  return visible.map((element) => element.textContent ?? "").join("\n");
}

describe("JXCG 应用壳与五页面（浏览器 mock）", () => {
  it("启动加载 mock 目录，默认进入计算页并可展开/收起导航", async () => {
    await mount();
    expect(host.textContent).toContain("JXCG · 剑网3 交易行计算器");
    expect(host.textContent).toContain("浏览器预览");
    // 状态栏展示数据版本 + 数据范围 note（catalog.banben.note）
    expect(host.textContent).toContain("浏览器 mock 预览数据");
    expect(host.textContent).toContain("配方模式");
    expect(host.textContent).toContain("云芝竹沥羹");

    // 导航默认收起（只有图标），展开后显示文字
    const toggle = host.querySelector<HTMLButtonElement>('button[title="展开导航"]');
    expect(toggle).not.toBeNull();
    await act(async () => toggle!.click());
    expect(host.textContent).toContain("行情");
    expect(host.querySelector('button[title="收起导航"]')).not.toBeNull();
  });

  it("五个页面可切换、页面保活，主题切换生效", async () => {
    await mount();

    // 计算页输入配方搜索
    const search = host.querySelector<HTMLInputElement>('input[aria-label="搜索配方"]')!;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(search, "云芝");
      search.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await flush(250);
    expect(visiblePageText()).toContain("云芝竹沥羹");

    // 行情页
    await clickButton("行情");
    expect(visiblePageText()).toContain("物品价格");
    expect(visiblePageText()).toContain("珠贝母");
    expect(visiblePageText()).toContain("保存快照");

    // 五行石页（mock 精炼计算）
    await clickButton("五行石");
    expect(visiblePageText()).toContain("转换配方");
    expect(visiblePageText()).toContain("期望投入次数");
    expect(visiblePageText()).toContain("精炼结果");

    // 排行页（mock 排名计算 + 图表）
    await clickButton("排行");
    expect(visiblePageText()).toContain("排行表");
    expect(visiblePageText()).toContain("TOP");

    // 设置页
    await clickButton("设置");
    expect(visiblePageText()).toContain("服务器与值集");
    expect(visiblePageText()).toContain("数据版本");
    expect(visiblePageText()).toContain("关于");

    // 回到计算页：保活的输入仍在
    await clickButton("计算");
    const searchAgain = host.querySelector<HTMLInputElement>('input[aria-label="搜索配方"]')!;
    expect(searchAgain.value).toBe("云芝");

    // 主题切换
    const themeButton = host.querySelector<HTMLButtonElement>('button[aria-label="切换主题"]')!;
    await act(async () => themeButton.click());
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    await act(async () => themeButton.click());
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });

  it("手动模式：净收入/保本价端到端计算（PLAN §6.4 口径）", async () => {
    await mount();
    const manual = [...host.querySelectorAll("button")].find((button) => button.textContent === "手动模式");
    expect(manual).toBeDefined();
    await act(async () => manual!.click());
    await flush(400);
    const text = visiblePageText();
    expect(text).toContain("到手金额");
    expect(text).toContain("1139金20银");
    expect(text).toContain("保本总售价");
  });

  it("行情页一键刷新：进度事件、价格与自动快照落盘", async () => {
    await mount();
    await clickButton("行情");
    expect(host.querySelectorAll('[class*="snapshotItem"]')).toHaveLength(0);

    const refreshButton = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("一键刷新"),
    );
    expect(refreshButton).toBeDefined();
    // task-8：全量刷新文案（数量 + 预计时长），不再有 200 截断
    expect(refreshButton!.textContent).toMatch(/一键刷新（\d+）/);
    expect(refreshButton!.title).toContain("全部相关物品");
    expect(refreshButton!.title).not.toContain("200");
    await act(async () => refreshButton!.click());
    await flush(2000);

    expect(host.textContent).toContain("行情刷新完成");
    expect(host.querySelectorAll('[class*="snapshotItem"]').length).toBeGreaterThan(0);
  });

  it("排行页点行跳转计算页并选中该配方", async () => {
    await mount();
    await clickButton("排行");
    await flush(500);
    const row = [...host.querySelectorAll<HTMLTableRowElement>("tbody tr")].find((element) =>
      (element.textContent ?? "").includes("寒铁护腕"),
    );
    expect(row, "排行表应包含有价的「寒铁护腕」").toBeDefined();
    await act(async () => row!.click());
    await flush(500);
    // 已切回计算页，且配方列表选中寒铁护腕
    expect(host.textContent).toContain("材料表");
    const selected = host.querySelector('[role="option"][aria-selected="true"]');
    expect(selected?.textContent).toContain("寒铁护腕");
  });
});
