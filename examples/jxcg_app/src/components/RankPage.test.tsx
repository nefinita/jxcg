import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import RankPage from "./RankPage";
import { mockCatalog, mockPriceMap } from "../api/mock";
import { DEFAULT_SETTINGS, DEFAULT_SHOUXUFEI } from "../utils/constants";
import type { JiageBiao } from "../types";

beforeAll(() => {
  class ResizeObserverMock {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  Object.assign(globalThis, { ResizeObserver: ResizeObserverMock });
});

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

async function render() {
  await act(async () =>
    root.render(
      <RankPage
        catalog={mockCatalog()}
        settings={DEFAULT_SETTINGS}
        shouxufei={DEFAULT_SHOUXUFEI}
        prices={makePrices()}
        fuwuqiId="jiandanqinxin"
        fuwuqiName="剑胆琴心"
        onOpenRecipe={vi.fn()}
        addToast={vi.fn()}
        setStatus={vi.fn()}
      />,
    ),
  );
  await flush();
}

function rowTexts(): string[] {
  return [...host.querySelectorAll("tbody tr")].map((row) => row.textContent ?? "");
}

async function toggleIncludeZhongjian() {
  const label = [...host.querySelectorAll("label")].find((element) =>
    (element.textContent ?? "").includes("包含中间产物"),
  );
  expect(label, "找不到「包含中间产物」勾选").toBeDefined();
  const input = label!.querySelector("input")!;
  await act(async () => input.click());
  await flush();
}

describe("排行页中间产物排除（task-6）", () => {
  it("默认排除只产中间产物的配方", async () => {
    await render();
    expect(host.textContent).toContain("排行表");
    expect(rowTexts().some((row) => row.includes("酱料制作"))).toBe(false);
    expect(rowTexts().some((row) => row.includes("锦缎织造"))).toBe(false);
    // 正常成品配方仍在
    expect(rowTexts().some((row) => row.includes("寒铁护腕"))).toBe(true);
  });

  it("勾选「包含中间产物」后返回这些行并打标", async () => {
    await render();
    await toggleIncludeZhongjian();
    const rows = rowTexts();
    const zjRow = rows.find((row) => row.includes("酱料制作"));
    expect(zjRow).toBeDefined();
    expect(zjRow!).toContain("中间产物");
  });
});
