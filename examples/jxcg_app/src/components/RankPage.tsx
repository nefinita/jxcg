import { useEffect, useMemo, useRef, useState } from "react";
import { Bar, BarChart, CartesianGrid, Cell, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import * as api from "../api/commands";
import { IconRefresh, IconTrend } from "./icons";
import clsx from "../utils/clsx";
import common from "../styles/common.module.css";
import styles from "./RankPage.module.css";
import { usePersistentState } from "../hooks/usePersistentState";
import { DEFAULT_RANK_PREFS, rankPrefsSchema } from "../state/schemas";
import { JIYI_LABEL, JIYI_LIST, PAIHANG_LABEL, PAIHANG_LIST, STORAGE_KEYS } from "../utils/constants";
import { formatBp, formatQiwang } from "../utils/format";
import { formatTong } from "../utils/money";
import { danjiaOf, quanliangJiageMap, shoudongIdsOf } from "../utils/jiage";
import type { Catalog, JiageBiao, PaihangRow, PaihangZhibiao, Settings, Shouxufei } from "../types";
import type { Toast } from "../hooks/useToast";

export interface RankPageProps {
  catalog: Catalog;
  settings: Settings;
  shouxufei: Shouxufei;
  prices: JiageBiao;
  fuwuqiId: string;
  fuwuqiName: string;
  onOpenRecipe: (peifangId: string) => void;
  addToast: (message: string, type?: Toast["type"]) => void;
  setStatus: (message: string) => void;
}

/** 按指标把排行值换算成图表用数字（金额用「金」，利润率用百分数）。 */
function chartValue(row: PaihangRow, zhibiao: PaihangZhibiao): number {
  if (zhibiao === "lirun") return row.lirun / 10_000;
  if (zhibiao === "mei_jingli") return row.mei_jingli / 10_000;
  return row.lirunlv_bp / 100;
}

/** 图表 tooltip：把换算后的数值还原成完整口径。 */
function chartTooltip(value: number, zhibiao: PaihangZhibiao): string {
  if (zhibiao === "lirunlv") return formatBp(Number(value) * 100);
  return formatTong(Math.round(Number(value) * 10_000));
}

function axisFormat(value: number, zhibiao: PaihangZhibiao): string {
  if (zhibiao === "lirunlv") return `${Number(value).toFixed(0)}%`;
  const num = Number(value);
  return `${num.toFixed(num >= 100 || num <= -100 ? 0 : 1)}金`;
}

export default function RankPage({
  catalog,
  settings,
  shouxufei,
  prices,
  fuwuqiId,
  fuwuqiName,
  onOpenRecipe,
  addToast,
  setStatus,
}: RankPageProps) {
  const [prefs, setPrefs] = usePersistentState(STORAGE_KEYS.rankPrefs, () => DEFAULT_RANK_PREFS, rankPrefsSchema);
  const [rows, setRows] = useState<PaihangRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [recalcNonce, setRecalcNonce] = useState(0);
  const rankRev = useRef(0);

  // 参与计算的物品（配方材料 + 产物去重），用于行情完整度提示。
  const relevantIds = useMemo(() => {
    const ids = new Set<string>();
    for (const peifang of catalog.peifang) {
      for (const item of peifang.cailiao) ids.add(item.wupin_id);
      for (const slot of peifang.chuchan) ids.add(slot.wupin_id);
    }
    return [...ids];
  }, [catalog.peifang]);

  const completeness = useMemo(() => {
    let priced = 0;
    for (const id of relevantIds) {
      const wupin = catalog.wupin.find((entry) => entry.id === id);
      if (!wupin) continue;
      const price = danjiaOf(prices, fuwuqiId, wupin);
      if (price !== null) priced += 1;
    }
    return { priced, total: relevantIds.length };
  }, [relevantIds, catalog.wupin, prices, fuwuqiId]);

  useEffect(() => {
    const rev = ++rankRev.current;
    setLoading(true);
    setError("");
    const timer = window.setTimeout(() => {
      api
        .rankRecipes({
          jiyi: prefs.jiyi === "all" ? null : prefs.jiyi,
          jingli_danjia: settings.jingli_danjia,
          shouxufei,
          // 全量行情 + 手填价（递归中间产物采购取价用）；完整度展示仍按 relevantIds 自算。
          jiage: quanliangJiageMap(prices, fuwuqiId),
          shoudong: shoudongIdsOf(prices, fuwuqiId),
          zhibiao: prefs.zhibiao,
          jiyi_dengji: settings.jiyi_dengji,
          limit: null,
          baohan_quejia: prefs.baohan_quejia,
          zhongjian_zizhi: settings.zhongjian_zizhi ?? true,
          baohan_zhongjian: prefs.baohan_zhongjian,
        })
        .then((data) => {
          if (rev !== rankRev.current) return;
          setRows(data);
          setLoading(false);
        })
        .catch((err: unknown) => {
          if (rev !== rankRev.current) return;
          setError(err instanceof Error ? err.message : String(err));
          setLoading(false);
        });
    }, 140);
    return () => {
      window.clearTimeout(timer);
      ++rankRev.current;
    };
  }, [
    prefs.jiyi,
    prefs.zhibiao,
    prefs.baohan_quejia,
    prefs.baohan_zhongjian,
    settings.jingli_danjia,
    settings.jiyi_dengji,
    settings.zhongjian_zizhi,
    shouxufei,
    prices,
    fuwuqiId,
    recalcNonce,
  ]);

  const chartRows = useMemo(
    () =>
      rows.slice(0, Math.max(1, prefs.limit)).map((row) => ({
        mingcheng: row.mingcheng,
        value: chartValue(row, prefs.zhibiao),
        negative: (prefs.zhibiao === "lirunlv" ? row.lirunlv_bp : prefs.zhibiao === "lirun" ? row.lirun : row.mei_jingli) < 0,
        quejia: row.quejia,
      })),
    [rows, prefs.limit, prefs.zhibiao],
  );

  const openRecipe = (row: PaihangRow) => {
    onOpenRecipe(row.peifang_id);
    setStatus(`已跳转计算页：${row.mingcheng}`);
  };

  const handleRefresh = () => {
    setStatus("排行已按当前行情重算");
    addToast("排行已重算", "info");
    setRecalcNonce((value) => value + 1);
  };

  return (
    <div className={common.page}>
      <div className={common.pageHeader}>
        <div>
          <h1 className={common.pageTitle}>排行</h1>
          <p className={common.pageSub}>
            {fuwuqiName} · 行情完整度 {completeness.priced}/{completeness.total} · 精力单价{" "}
            {formatTong(settings.jingli_danjia)}（设置页可改）
          </p>
        </div>
        <div className={common.toolbar}>
          <div className={common.segmented} role="group" aria-label="排行指标">
            {PAIHANG_LIST.map((zhibiao) => (
              <button
                key={zhibiao.id}
                className={clsx(common.segmentedBtn, prefs.zhibiao === zhibiao.id && common.segmentedActive)}
                aria-pressed={prefs.zhibiao === zhibiao.id}
                title={zhibiao.hint}
                onClick={() => setPrefs((previous) => ({ ...previous, zhibiao: zhibiao.id }))}
              >
                {zhibiao.label}
              </button>
            ))}
          </div>
          <button className={common.btn} onClick={handleRefresh} title="按最新行情重新计算排行">
            <IconRefresh size={14} />
            重算
          </button>
        </div>
      </div>

      <section className={common.card}>
        <div className={styles.controls}>
          <label className={common.fieldRow}>
            <span className={common.fieldLabel}>技艺</span>
            <select
              className={clsx(common.select, styles.select)}
              value={prefs.jiyi}
              onChange={(event) => setPrefs((previous) => ({ ...previous, jiyi: event.target.value as typeof previous.jiyi }))}
              aria-label="技艺筛选"
            >
              <option value="all">全部技艺</option>
              {JIYI_LIST.map((jiyi) => (
                <option key={jiyi.id} value={jiyi.id}>
                  {jiyi.label}
                </option>
              ))}
            </select>
          </label>
          <label className={common.fieldRow}>
            <span className={common.fieldLabel}>TOP</span>
            <select
              className={clsx(common.select, styles.select)}
              value={prefs.limit}
              onChange={(event) => setPrefs((previous) => ({ ...previous, limit: Number(event.target.value) || 10 }))}
              aria-label="TOP N"
            >
              {[10, 20, 50].map((limit) => (
                <option key={limit} value={limit}>
                  {limit}
                </option>
              ))}
            </select>
          </label>
          <label className={common.checkboxRow}>
            <input
              type="checkbox"
              checked={prefs.baohan_quejia}
              onChange={(event) => setPrefs((previous) => ({ ...previous, baohan_quejia: event.target.checked }))}
            />
            包含缺价配方
          </label>
          <label className={common.checkboxRow} title="只产中间产物的配方默认不参与成品排行">
            <input
              type="checkbox"
              checked={prefs.baohan_zhongjian}
              onChange={(event) => setPrefs((previous) => ({ ...previous, baohan_zhongjian: event.target.checked }))}
            />
            包含中间产物
          </label>
          <span className={styles.controlsSpacer} />
          <span className={common.hint}>
            行情完整度 {completeness.priced}/{completeness.total}
          </span>
          <span className={common.progressTrack}>
            <span
              className={common.progressFill}
              style={{ width: `${completeness.total ? Math.round((completeness.priced / completeness.total) * 100) : 0}%` }}
            />
          </span>
        </div>
        <p className={common.hint}>
          {PAIHANG_LABEL[prefs.zhibiao]}排序 · 缺价配方默认不参与排行（可勾选查看）；只产中间产物的配方默认不进入成品排行
          （可勾选「包含中间产物」查看，标记为中间产物）；点行跳转计算页。
          {loading ? " 计算中…" : ""}
        </p>
        {error && <div className={common.noticeDanger}>{error}</div>}
      </section>

      <div className={styles.layout}>
        <section className={clsx(common.card, styles.tableCard)}>
          <div className={common.cardTitle}>
            <span>排行表</span>
            <span className={common.hint}>{rows.length} 个配方</span>
          </div>
          <div className={common.tableWrap}>
            <table className={common.table}>
              <thead>
                <tr>
                  <th className={common.center}>#</th>
                  <th>配方</th>
                  <th>技艺</th>
                  <th className={common.num}>净利润</th>
                  <th className={common.num}>利润率</th>
                  <th className={common.num}>每精力</th>
                  <th className={common.num}>精力</th>
                  <th className={common.center}>状态</th>
                </tr>
              </thead>
              <tbody>
                {rows.length === 0 && (
                  <tr>
                    <td colSpan={8}>
                      <p className={common.empty}>
                        {loading ? "计算中…" : "没有可排行的配方：请先在行情页录价，或勾选「包含缺价配方」。"}
                      </p>
                    </td>
                  </tr>
                )}
                {rows.map((row, index) => {
                  const active = prefs.zhibiao;
                  return (
                    <tr
                      key={row.peifang_id}
                      className={clsx(common.rowButton, row.quejia && styles.rowQuejia)}
                      onClick={() => openRecipe(row)}
                      title="点击跳转计算页"
                    >
                      <td className={common.center}>{index + 1}</td>
                      <td>{row.mingcheng}</td>
                      <td>{JIYI_LABEL[row.jiyi]}</td>
                      <td className={clsx(common.num, active === "lirun" && styles.metricActive, row.lirun < 0 && common.negative)}>
                        {formatTong(row.lirun)}
                      </td>
                      <td
                        className={clsx(
                          common.num,
                          active === "lirunlv" && styles.metricActive,
                          row.lirunlv_bp < 0 && common.negative,
                        )}
                      >
                        {formatBp(row.lirunlv_bp)}
                      </td>
                      <td
                        className={clsx(
                          common.num,
                          active === "mei_jingli" && styles.metricActive,
                          row.mei_jingli < 0 && common.negative,
                        )}
                      >
                        {formatTong(row.mei_jingli)}
                      </td>
                      <td className={common.num}>{row.jingli}</td>
                      <td className={common.center}>
                        <span className={styles.statusBadges}>
                          {row.zhongjian && <span className={common.badgeZhongjian}>中间产物</span>}
                          {row.quejia ? (
                            <span className={common.badgeQuejia}>缺价</span>
                          ) : (
                            !row.zhongjian && <span className={common.badgeHangqing}>有价</span>
                          )}
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </section>

        <section className={clsx(common.card, styles.chartCard)}>
          <div className={common.cardTitle}>
            <span>
              <IconTrend size={14} /> TOP {Math.max(1, prefs.limit)} · {PAIHANG_LABEL[prefs.zhibiao]}
            </span>
            <span className={common.hint}>{prefs.jiyi === "all" ? "全部技艺" : JIYI_LABEL[prefs.jiyi]}</span>
          </div>
          {chartRows.length === 0 ? (
            <p className={common.empty}>暂无可绘制的排行数据。</p>
          ) : (
            <div className={styles.chart} style={{ height: Math.max(220, chartRows.length * 38) }}>
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={chartRows} layout="vertical" margin={{ top: 4, right: 18, bottom: 4, left: 4 }}>
                  <CartesianGrid stroke="var(--border-subtle)" strokeDasharray="3 3" horizontal={false} />
                  <XAxis
                    type="number"
                    tickFormatter={(value) => axisFormat(Number(value), prefs.zhibiao)}
                    tick={{ fontSize: 11, fill: "var(--text-muted)" }}
                  />
                  <YAxis
                    type="category"
                    dataKey="mingcheng"
                    width={104}
                    tick={{ fontSize: 12, fill: "var(--text-secondary)" }}
                  />
                  <Tooltip
                    formatter={(value) => chartTooltip(Number(value), prefs.zhibiao)}
                    cursor={{ fill: "var(--glass-highlight)" }}
                    contentStyle={{
                      background: "var(--bg-surface)",
                      border: "1px solid var(--border-default)",
                      borderRadius: 8,
                      fontSize: 12,
                    }}
                  />
                  <Bar dataKey="value" name={PAIHANG_LABEL[prefs.zhibiao]} radius={[0, 4, 4, 0]} maxBarSize={22}>
                    {chartRows.map((entry) => (
                      <Cell key={entry.mingcheng} fill={entry.negative ? "#ef4444" : entry.quejia ? "#94a3b8" : "#14b8a6"} />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            </div>
          )}
          <p className={common.hint}>
            图表数值：{prefs.zhibiao === "lirunlv" ? "利润率百分比" : `${PAIHANG_LABEL[prefs.zhibiao]}（金）`}
            {rows.some((row) => row.quejia) ? " · 灰色柱为缺价配方" : ""}
          </p>
        </section>
      </div>
      {loading && <p className={common.hint}>正在按最新行情排行…</p>}
      <p className={common.hint}>
        期望产出按权重分母 2^20 折算；净利润 = Σ(产出×售价×(1−手续费率)) − 材料 − 精力 − 制作费 − 保管费。
      </p>
    </div>
  );
}
