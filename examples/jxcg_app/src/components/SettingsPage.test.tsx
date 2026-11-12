import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";
import { mockCatalog } from "../api/mock";
import { DEFAULT_SETTINGS, DEFAULT_SHOUXUFEI } from "../utils/constants";
import type { Settings } from "../types";

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

async function render(settings: Settings, updateSettings = vi.fn()) {
  await act(async () =>
    root.render(
      <SettingsPage
        catalog={mockCatalog()}
        settings={settings}
        updateSettings={updateSettings}
        shouxufei={DEFAULT_SHOUXUFEI}
        zhijiOverride={null}
        setZhijiOverride={vi.fn()}
        theme="light"
        setTheme={vi.fn()}
        onSaveSettings={async () => {}}
        onReloadCatalog={async () => {}}
        onExportArchive={vi.fn()}
        onImportArchive={async () => {}}
        onResetSettings={vi.fn()}
        addToast={vi.fn()}
        setStatus={vi.fn()}
      />,
    ),
  );
}

function zhongjianCheckbox(): HTMLInputElement {
  const label = [...host.querySelectorAll("label")].find((element) =>
    (element.textContent ?? "").includes("中间产物按自制成本摊入"),
  );
  expect(label, "找不到中间产物开关").toBeDefined();
  return label!.querySelector("input")!;
}

describe("设置页：中间产物开关（task-6）", () => {
  it("默认开启，切换后写回 settings.zhongjian_zizhi=false", async () => {
    const updateSettings = vi.fn();
    await render(DEFAULT_SETTINGS, updateSettings);
    const checkbox = zhongjianCheckbox();
    expect(checkbox.checked).toBe(true);
    await act(async () => checkbox.click());
    expect(updateSettings).toHaveBeenCalledWith({ zhongjian_zizhi: false });
  });

  it("数据版本区展示数据范围 note（task-8）", async () => {
    await render(DEFAULT_SETTINGS);
    expect(host.textContent).toContain("数据范围");
    expect(host.textContent).toContain("50 级");
    expect(host.textContent).toContain(mockCatalog().banben.note);
  });

  it("旧设置缺字段时按默认开启展示", async () => {
    const settings = { ...DEFAULT_SETTINGS } as Settings;
    delete (settings as Partial<Settings>).zhongjian_zizhi;
    await render(settings);
    expect(zhongjianCheckbox().checked).toBe(true);
  });
});
