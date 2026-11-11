import { useState } from "react";
import MoneyInput from "./MoneyInput";
import { IconDownload, IconRefresh, IconUpload } from "./icons";
import clsx from "../utils/clsx";
import common from "../styles/common.module.css";
import styles from "./SettingsPage.module.css";
import type { Catalog, Settings, Shouxufei } from "../types";
import type { Theme } from "../hooks/useTheme";
import type { Toast } from "../hooks/useToast";
import { APP_VERSION, GITHUB_ISSUES_URL, QUZHENG_LIST, QUZHENG_LABEL } from "../utils/constants";
import { formatBp } from "../utils/format";
import { formatTong } from "../utils/money";

export interface SettingsPageProps {
  catalog: Catalog;
  settings: Settings;
  updateSettings: (patch: Partial<Settings>) => void;
  /** 当前生效值集（用户覆盖 > 选中值集 > 默认） */
  shouxufei: Shouxufei;
  zhijiOverride: Shouxufei | null;
  setZhijiOverride: (value: Shouxufei | null) => void;
  theme: Theme;
  setTheme: (theme: Theme) => void;
  onSaveSettings: () => Promise<void>;
  onReloadCatalog: () => Promise<void>;
  onExportArchive: () => void;
  onImportArchive: () => Promise<void>;
  onResetSettings: () => void;
  addToast: (message: string, type?: Toast["type"]) => void;
  setStatus: (message: string) => void;
}

/** 设置页：服务器 / 值集费率 / 精力单价 / 技艺等级 / 自动拉价 / 主题 / 数据版本 / 存档导入导出 / 关于。 */
export default function SettingsPage({
  catalog,
  settings,
  updateSettings,
  shouxufei,
  zhijiOverride,
  setZhijiOverride,
  theme,
  setTheme,
  onSaveSettings,
  onReloadCatalog,
  onExportArchive,
  onImportArchive,
  onResetSettings,
  addToast,
  setStatus,
}: SettingsPageProps) {
  const [saving, setSaving] = useState(false);
  const [reloading, setReloading] = useState(false);
  const [importing, setImporting] = useState(false);

  const zhiji = catalog.zhiji.find((entry) => entry.id === settings.zhiji_id) ?? catalog.zhiji[0];
  const customZhiji = zhijiOverride !== null;
  const fuwuqi = catalog.fuwuqi.find((entry) => entry.id === settings.fuwuqi_id);

  const updateShouxufei = (patch: Partial<Shouxufei>) => {
    setZhijiOverride({ ...shouxufei, ...patch });
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await onSaveSettings();
    } finally {
      setSaving(false);
    }
  };

  const handleReload = async () => {
    setReloading(true);
    try {
      await onReloadCatalog();
    } finally {
      setReloading(false);
    }
  };

  const handleImport = async () => {
    setImporting(true);
    try {
      await onImportArchive();
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className={common.page}>
      <div className={common.pageHeader}>
        <div>
          <h1 className={common.pageTitle}>设置</h1>
          <p className={common.pageSub}>
            当前服务器 {fuwuqi?.mingcheng ?? settings.fuwuqi_id} · 值集 {zhiji?.mingcheng ?? settings.zhiji_id} · 修改会自动保存
          </p>
        </div>
        <div className={common.toolbar}>
          <button className={common.btn} onClick={handleSave} disabled={saving}>
            <IconDownload size={14} />
            {saving ? "保存中…" : "立即保存设置"}
          </button>
          <button className={common.btn} onClick={onResetSettings}>
            恢复默认设置
          </button>
        </div>
      </div>

      <div className={common.grid2}>
        {/* 服务器与值集 */}
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>服务器与值集</span>
            <span className={common.hint}>{customZhiji ? "已自定义费率" : "使用值集默认"}</span>
          </div>
          <div className={styles.form}>
            <label className={common.field}>
              <span className={common.fieldLabel}>服务器（行情按服隔离）</span>
              <select
                className={common.select}
                value={settings.fuwuqi_id}
                onChange={(event) => {
                  updateSettings({ fuwuqi_id: event.target.value });
                  const next = catalog.fuwuqi.find((entry) => entry.id === event.target.value);
                  setStatus(`服务器已切换为 ${next?.mingcheng ?? event.target.value}`);
                }}
              >
                {catalog.fuwuqi.map((entry) => (
                  <option key={entry.id} value={entry.id}>
                    {entry.mingcheng}
                    {entry.daju ? ` · ${entry.daju}` : ""}
                  </option>
                ))}
              </select>
            </label>

            <label className={common.field}>
              <span className={common.fieldLabel}>值集（费率/取整/保管费）</span>
              <select
                className={common.select}
                value={settings.zhiji_id}
                onChange={(event) => {
                  updateSettings({ zhiji_id: event.target.value });
                  setZhijiOverride(null);
                }}
              >
                {catalog.zhiji.map((entry) => (
                  <option key={entry.id} value={entry.id}>
                    {entry.mingcheng}
                  </option>
                ))}
              </select>
            </label>
            {zhiji?.note && <p className={common.hint}>{zhiji.note}</p>}

            <div className={styles.rateGrid}>
              <label className={common.field}>
                <span className={common.fieldLabel}>手续费率（%）</span>
                <input
                  className={common.numberInput}
                  type="number"
                  min={0}
                  max={99.99}
                  step={0.01}
                  value={Number((shouxufei.shuilv_bp / 100).toFixed(2))}
                  onChange={(event) => {
                    const percent = Number(event.target.value);
                    if (!Number.isFinite(percent)) return;
                    const bp = Math.min(9_999, Math.max(0, Math.round(percent * 100)));
                    updateShouxufei({ shuilv_bp: bp });
                  }}
                />
              </label>
              <label className={common.field}>
                <span className={common.fieldLabel}>取整方式</span>
                <select
                  className={common.select}
                  value={shouxufei.quzheng}
                  onChange={(event) =>
                    updateShouxufei({ quzheng: event.target.value as Shouxufei["quzheng"] })
                  }
                >
                  {QUZHENG_LIST.map((entry) => (
                    <option key={entry.id} value={entry.id}>
                      {entry.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className={common.field}>
                <span className={common.fieldLabel}>保管费（每次挂单）</span>
                <MoneyInput
                  ariaLabel="保管费"
                  value={shouxufei.baoguanfei}
                  onChange={(tong) => updateShouxufei({ baoguanfei: tong })}
                />
              </label>
            </div>
            <div className={common.toolbar}>
              <button className={common.btn} onClick={() => setZhijiOverride(null)} disabled={!customZhiji}>
                恢复所选值集默认
              </button>
              <span className={common.hint}>
                生效：手续费 {formatBp(shouxufei.shuilv_bp)}（{QUZHENG_LABEL[shouxufei.quzheng]}）+ 保管费{" "}
                {formatTong(shouxufei.baoguanfei)}/次
              </span>
            </div>
          </div>
        </section>

        {/* 计算参数 */}
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>计算参数</span>
            <span className={common.hint}>影响计算页与排行</span>
          </div>
          <div className={styles.form}>
            <label className={common.field}>
              <span className={common.fieldLabel}>精力单价（每点精力折合铜，0 = 不计精力成本）</span>
              <MoneyInput
                ariaLabel="精力单价"
                value={settings.jingli_danjia}
                onChange={(tong) => updateSettings({ jingli_danjia: tong })}
              />
            </label>
            <label className={common.field}>
              <span className={common.fieldLabel}>技艺等级（0–50，影响产出加成）</span>
              <div className={common.fieldRow}>
                <input
                  className={styles.range}
                  type="range"
                  min={0}
                  max={50}
                  value={settings.jiyi_dengji}
                  onChange={(event) => updateSettings({ jiyi_dengji: Number(event.target.value) || 0 })}
                  aria-label="技艺等级"
                />
                <span className={styles.rangeValue}>{settings.jiyi_dengji}</span>
              </div>
              <span className={common.hint}>当前为展示加成参数（后端暂按 0 加成计算）。</span>
            </label>
            <label className={common.checkboxRow}>
              <input
                type="checkbox"
                checked={settings.auto_refresh}
                onChange={(event) => updateSettings({ auto_refresh: event.target.checked })}
              />
              启动时自动联网拉价（JX3BOX）
            </label>
            <p className={common.hint}>
              精力消耗按「精力消耗 × 精力单价」计入总成本；行情拉取失败时会保留最近一次价格（last-known-good）。
            </p>
          </div>
        </section>

        {/* 主题 */}
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>外观</span>
            <span className={common.hint}>深浅色主题</span>
          </div>
          <div className={common.segmented} role="group" aria-label="主题">
            <button
              className={clsx(common.segmentedBtn, theme === "light" && common.segmentedActive)}
              aria-pressed={theme === "light"}
              onClick={() => setTheme("light")}
            >
              浅色
            </button>
            <button
              className={clsx(common.segmentedBtn, theme === "dark" && common.segmentedActive)}
              aria-pressed={theme === "dark"}
              onClick={() => setTheme("dark")}
            >
              深色
            </button>
          </div>
          <p className={common.hint}>主题与 jpcg 一致（薄荷/松石绿 token），选择会写入本地存储。</p>
        </section>

        {/* 数据 */}
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>数据版本</span>
            <span className={common.hint}>随包 TOML</span>
          </div>
          <dl className={styles.dataList}>
            <div>
              <dt>版本</dt>
              <dd>
                {catalog.banben.level} 级 · 第 {catalog.banben.season} 赛季 · {catalog.banben.modified}
              </dd>
            </div>
            <div>
              <dt>数据量</dt>
              <dd>
                物品 {catalog.wupin.length} · 配方 {catalog.peifang.length} · 五行石 {catalog.wuxingshi.length} · 服务器{" "}
                {catalog.fuwuqi.length} · 值集 {catalog.zhiji.length}
              </dd>
            </div>
          </dl>
          {catalog.banben.note && <p className={common.hint}>{catalog.banben.note}</p>}
          <div className={common.toolbar}>
            <button className={common.btn} onClick={handleReload} disabled={reloading}>
              <IconRefresh size={14} />
              {reloading ? "重新加载中…" : "重新加载数据目录"}
            </button>
            <span className={common.hint}>数据目录可用 JXCG_DATA_DIR 覆盖（桌面端）</span>
          </div>
        </section>

        {/* 存档导入导出 */}
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>存档导入 / 导出</span>
            <span className={common.hint}>设置 + 行情 + 收藏 + 快照</span>
          </div>
          <div className={common.toolbar}>
            <button className={common.btn} onClick={onExportArchive}>
              <IconDownload size={14} />
              导出存档 JSON
            </button>
            <button className={common.btn} onClick={() => void handleImport()} disabled={importing}>
              <IconUpload size={14} />
              {importing ? "导入中…" : "导入存档 JSON"}
            </button>
          </div>
          <p className={common.hint}>
            存档 JSON 与行情页的行情 JSON 是两种格式；导入存档会覆盖当前设置、行情缓存、收藏与快照，请先导出备份。
          </p>
        </section>

        {/* 关于 */}
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>关于</span>
            <span className={common.hint}>v{APP_VERSION}</span>
          </div>
          <dl className={styles.dataList}>
            <div>
              <dt>名称</dt>
              <dd>JXCG · 剑网3 交易行计算器</dd>
            </div>
            <div>
              <dt>许可</dt>
              <dd>GPL-3.0</dd>
            </div>
            <div>
              <dt>行情来源</dt>
              <dd>JX3BOX（社区接口，非官方、无 SLA；失败保留最近缓存价）</dd>
            </div>
            <div>
              <dt>费用口径</dt>
              <dd>5% HALF_UP + 保管费 80 银/24h（可在值集中修改）</dd>
            </div>
            <div>
              <dt>计算模型</dt>
              <dd>产出权重分母 2^20；金额全程整数铜；期望产出按权重折算</dd>
            </div>
          </dl>
          <a className={common.btn} href={GITHUB_ISSUES_URL} target="_blank" rel="noreferrer">
            反馈 / Issues
          </a>
        </section>
      </div>

      <p className={common.hint}>
        设置会自动保存（桌面端写入应用数据目录 settings.toml）；值集的自定义费率仅保存在本机浏览器存储中，可通过「恢复所选值集默认」清除。
      </p>
    </div>
  );
}
