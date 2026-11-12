import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import WuxingshiLianPanel from "./WuxingshiLianPanel";
import { mockCatalog, mockPriceMap } from "../api/mock";
import { DEFAULT_SETTINGS, DEFAULT_SHOUXUFEI } from "../utils/constants";
import type { JiageBiao } from "../types";

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

async function flush(ms = 700) {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

async function render(omit: string[] = []) {
  await act(async () =>
    root.render(
      <WuxingshiLianPanel
        catalog={mockCatalog()}
        settings={DEFAULT_SETTINGS}
        updateSettings={vi.fn()}
        shouxufei={DEFAULT_SHOUXUFEI}
        prices={makePrices(omit)}
        fuwuqiId="jiandanqinxin"
        fuwuqiName="剑胆琴心"
        setStatus={vi.fn()}
      />,
    ),
  );
  await flush();
}

function text(): string {
  return host.textContent ?? "";
}

/** React 受控输入：原生 setter + input 事件。 */
async function typeInto(label: string, value: string) {
  const input = host.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`);
  expect(input, `找不到输入框：${label}`).not.toBeNull();
  await act(async () => {
    input!.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, value);
    input!.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await flush(700);
}

async function selectValue(label: string, value: string) {
  const select = host.querySelector<HTMLSelectElement>(`select[aria-label="${label}"]`);
  expect(select, `找不到下拉：${label}`).not.toBeNull();
  await act(async () => {
    select!.value = value;
    select!.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await flush(700);
}

describe("五行石一键合成到六级（task-10）", () => {
  it("金标准：约 593 一级石 / 581 精力 / 67.97 金加工费，分步 3-6 级", async () => {
    await render();
    expect(text()).toContain("一键合成参数");
    expect(text()).toContain("一键合成 · 五行石（六级）");
    expect(text()).toContain("593"); // 期望一级石
    expect(text()).toContain("581 点"); // 精力
    expect(text()).toContain("67金97银4铜"); // 加工费 679704 铜
    expect(text()).toContain("599金43银48铜"); // 单颗净成本
    expect(text()).toContain("mianban_3");
    expect(text()).toContain("mianban_4");
    expect(text()).toContain("mianban_5");
    expect(text()).toContain("mianban_6");
    expect(text()).toContain("分步明细");
    // 副产物 2/5/6 级，8 级概率 0 默认忽略
    expect(text()).toContain("五行石（二级）");
    expect(text()).toContain("填 0 忽略 8 级暴击收益");
  });

  it("合 6 出 8 级概率：bp=5（0.05%）时期望 +12.8 金/颗", async () => {
    await render();
    await typeInto("合6出8级概率", "5");
    expect(text()).toContain("合 6 出 8 级：期望");
    expect(text()).toContain("12金80银76铜"); // 期望收益 128076 铜
    // 8 级也进入副产物表
    expect(text()).toContain("五行石（八级）");
  });

  it("目标数量切换：成本线性翻倍、单颗成本不变", async () => {
    await render();
    await typeInto("目标数量", "2");
    expect(text()).toContain("1198金86银96铜"); // 净总成本 ×2
    expect(text()).toContain("1186"); // 一级石 593×2
    expect(text()).toContain("599金43银48铜"); // 单颗净成本不变
  });

  it("缺价：一级石与八级无行情时提示缺价", async () => {
    await render(["5_24423", "5_24430"]);
    expect(text()).toContain("缺价：五行石（一级）");
    // 打开 8 级暴击概率后，八级缺价才进入期望计算
    await typeInto("合6出8级概率", "5");
    expect(text()).toContain("缺价：五行石（一级）、五行石（八级）");
  });

  it("目标切到五级：只走 3/4/5 三步", async () => {
    await render();
    await selectValue("目标物品", "5_24427");
    expect(text()).toContain("一键合成 · 五行石（五级）");
    expect(text()).toContain("mianban_5");
    expect(text()).not.toContain("mianban_6");
  });
});
