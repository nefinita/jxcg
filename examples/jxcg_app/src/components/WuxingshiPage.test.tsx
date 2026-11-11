import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import WuxingshiPage from "./WuxingshiPage";
import { mockCatalog, mockPriceMap } from "../api/mock";
import { DEFAULT_SETTINGS, DEFAULT_SHOUXUFEI } from "../utils/constants";
import type { JiageBiao } from "../types";

/** 用 mock 行情构造某服价格缓存；omit 用于制造缺价场景。 */
function makePrices(omit: string[] = []): JiageBiao {
  const biao: JiageBiao = {};
  for (const [wupinId, danjia] of Object.entries(mockPriceMap("jiandanqinxin"))) {
    if (omit.includes(wupinId)) continue;
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

async function flush(ms = 500) {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

async function render(options: { prices?: JiageBiao } = {}) {
  const catalog = mockCatalog();
  await act(async () =>
    root.render(
      <WuxingshiPage
        catalog={catalog}
        settings={DEFAULT_SETTINGS}
        updateSettings={() => {}}
        shouxufei={DEFAULT_SHOUXUFEI}
        prices={options.prices ?? makePrices()}
        fuwuqiId="jiandanqinxin"
        fuwuqiName="剑胆琴心"
        onOpenMarket={() => {}}
        setStatus={() => {}}
      />,
    ),
  );
  await flush();
}

function text(): string {
  return host.textContent ?? "";
}

function inputByLabel(label: string): HTMLInputElement {
  const input = host.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`);
  expect(input, `找不到输入框：${label}`).not.toBeNull();
  return input!;
}

/** 模拟用户输入（React 19 需要原生 value setter + input 事件）。 */
async function typeInto(label: string, value: string) {
  const input = inputByLabel(label);
  await act(async () => {
    input.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
  });
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await flush(500);
}

function optionByText(partial: string): HTMLButtonElement {
  const option = [...host.querySelectorAll<HTMLButtonElement>('[role="option"]')].find((element) =>
    (element.textContent ?? "").includes(partial),
  );
  expect(option, `找不到转换配方：${partial}`).toBeDefined();
  return option!;
}

describe("五行石精炼页", () => {
  it("默认选中第一条转换：按 cishu ÷ 成功率 折算期望投入与成本（PLAN P4 口径）", async () => {
    await render();
    // 一级→三级：p=100% → 期望投入 1 次；成本 9500×13 + 3900 = 12金74银
    expect(text()).toContain("转换配方");
    expect(text()).toContain("精炼结果");
    expect(text()).toContain("期望投入次数");
    expect(text()).toContain("100%");
    expect(text()).toContain("12金74银"); // 期望总成本
    expect(text()).toContain("7金80银"); // 期望收入（扣手续费前）
    expect(text()).toContain("负5金33银"); // 净利润 = 74100 − 127400
    expect(text()).toContain("-41.84%"); // 利润率（万分比转 %）
    // 两种石头价格从行情库带出
    expect(text()).toContain("五行石（一级）");
    expect(text()).toContain("五行石（三级）");
    expect(text()).toContain("行情");
  });

  it("切换 48.78% 配方与目标成功次数：期望投入按 1/p 变化", async () => {
    await render();
    await act(async () => optionByText("三级+一级").click());
    await flush(500);
    expect(text()).toContain("2.05"); // 1 / 0.4878
    expect(text()).toContain("9.25%"); // 净利润率
    expect(text()).toContain("1金64银85铜"); // 净利润 = 204876−10244−178147

    await typeInto("目标成功次数", "3");
    expect(text()).toContain("6.15"); // 3 / 0.4878
  });

  it("缺价标红并可手填：手填价进入请求，缺价提示消失", async () => {
    await render({ prices: makePrices(["5_24423"]) });
    expect(text()).toContain("缺价：五行石（一级）");
    expect(inputByLabel("低阶石单价").value).toBe("0铜");

    await typeInto("低阶石单价", "1金");
    expect(text()).toContain("手填");
    // 手填 1金 后成本 = 10000×13 + 3900 = 13金39银；净利润 = 74100 − 133900
    expect(text()).toContain("负5金98银");
    expect(text()).not.toContain("缺价：五行石（一级）");

    // 恢复行情价 → 重新回到缺价状态
    const restore = [...host.querySelectorAll("button")].find((button) => button.textContent === "恢复行情价");
    expect(restore).toBeDefined();
    await act(async () => restore!.click());
    await flush(500);
    expect(text()).toContain("缺价：五行石（一级）");
  });
});
