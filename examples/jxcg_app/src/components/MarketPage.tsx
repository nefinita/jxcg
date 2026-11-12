import { useCallback, useMemo, useState } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import * as api from "../api/commands";
import MoneyInput from "./MoneyInput";
import {
  IconCamera,
  IconClock,
  IconDownload,
  IconRefresh,
  IconStar,
  IconUpload,
} from "./icons";
import clsx from "../utils/clsx";
import common from "../styles/common.module.css";
import styles from "./MarketPage.module.css";
import { usePersistentState } from "../hooks/usePersistentState";
import { marketPrefsSchema, DEFAULT_MARKET_PREFS, isRecord } from "../state/schemas";
import { JIAGE_FILE_VERSION, PINLEI_LABEL, PINLEI_LIST, STORAGE_KEYS } from "../utils/constants";
import { formatShijian, formatShijianShort, nowSeconds } from "../utils/format";
import { formatTong, formatTongCompact } from "../utils/money";
import { danjiaOf, getJiage, trendOf } from "../utils/jiage";
import { parsePastePrices } from "../utils/marketPaste";
import { dateStamp, downloadJson, pickTextFile } from "../utils/file";
import type {
  Catalog,
  JiageBiao,
  JiageDaochuFile,
  Kuaizhao,
  RefreshProgress,
  Shouxufei,
  Tong,
  Wupin,
} from "../types";
import type { Toast } from "../hooks/useToast";

const LAIGUAN_LABEL: Record<string, string> = {
  hangqing: "行情",
  jx3box: "行情",
  mock: "行情",
  shoudong: "手填",
  shangdian: "商店",
  quejia: "缺价",
};

const LAIGUAN_BADGE: Record<string, string> = {
  hangqing: common.badgeHangqing,
  jx3box: common.badgeHangqing,
  mock: common.badgeHangqing,
  shoudong: common.badgeShoudong,
  shangdian: common.badgeShangdian,
  quejia: common.badgeQuejia,
};

/** 物品收藏 key（与配方收藏区分）。 */
export function wupinFavKey(id: string): string {
  return `wupin:${id}`;
}

function sourceLabel(laiyuan: string): string {
  return LAIGUAN_LABEL[laiyuan] ?? laiyuan;
}

export interface MarketPageProps {
  catalog: Catalog;
  fuwuqiId: string;
  fuwuqiName: string;
  shouxufei: Shouxufei;
  prices: JiageBiao;
  favorites: string[];
  onToggleFavorite: (key: string) => void;
  snapshots: Kuaizhao[];
  onSaveSnapshot: () => void;
  onDeleteSnapshot: (shijian: number) => void;
  onOverridePrice: (wupinId: string, danjia: Tong) => void;
  onImportPrices: (entries: Array<{ wupinId: string; danjia: Tong }>, source: string) => void;
  onRefresh: () => void;
  /** 本次「一键刷新」覆盖的物品数（五行石 + 全部配方材料与产物，去重、不截断） */
  refreshCount: number;
  refreshing: boolean;
  progress: RefreshProgress | null;
  addToast: (message: string, type?: Toast["type"]) => void;
  setStatus: (message: string) => void;
}

export default function MarketPage({
  catalog,
  fuwuqiId,
  fuwuqiName,
  shouxufei,
  prices,
  favorites,
  onToggleFavorite,
  snapshots,
  onSaveSnapshot,
  onDeleteSnapshot,
  onOverridePrice,
  onImportPrices,
  onRefresh,
  refreshCount,
  refreshing,
  progress,
  addToast,
  setStatus,
}: MarketPageProps) {
  const [prefs, setPrefs] = usePersistentState(STORAGE_KEYS.marketPrefs, () => DEFAULT_MARKET_PREFS, marketPrefsSchema);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState(0);
  const [pasteText, setPasteText] = useState("");

  // ---- 行数据：目录物品 + 当前行情（行情 > 商店价 > 缺价） ----
  const rows = useMemo(() => {
    const query = prefs.search.trim().toLowerCase();
    return catalog.wupin
      .filter((wupin) => {
        if (query && !wupin.mingcheng.toLowerCase().includes(query) && !wupin.id.toLowerCase().includes(query)) {
          return false;
        }
        if (prefs.pinlei !== "all" && wupin.pinlei !== prefs.pinlei) return false;
        if (prefs.only_fav && !favorites.includes(wupinFavKey(wupin.id))) return false;
        return true;
      })
      .map((wupin) => {
        const entry = getJiage(prices, fuwuqiId, wupin.id);
        const display = danjiaOf(prices, fuwuqiId, wupin);
        return {
          wupin,
          entry,
          danjia: display?.danjia ?? null,
          laiyuan: display?.laiyuan ?? "quejia",
        };
      });
  }, [catalog.wupin, prefs.search, prefs.pinlei, prefs.only_fav, favorites, prices, fuwuqiId]);

  // 全量数据（数百物品）下只渲染前 200 条，搜索/类别筛选可缩小范围。
  const visibleRows = useMemo(() => rows.slice(0, 200), [rows]);
  const pricedCount = rows.filter((row) => row.danjia !== null).length;
  const latestUpdate = useMemo(() => {
    let latest = 0;
    for (const wupin of catalog.wupin) {
      const entry = getJiage(prices, fuwuqiId, wupin.id);
      if (entry && entry.gengxin_shijian > latest) latest = entry.gengxin_shijian;
    }
    return latest;
  }, [catalog.wupin, prices, fuwuqiId]);

  const selectedId = prefs.selected_wupin_id;
  const selectedWupin = catalog.wupin.find((wupin) => wupin.id === selectedId) ?? null;
  const selectedDisplay = selectedWupin ? danjiaOf(prices, fuwuqiId, selectedWupin) : null;
  const trend = selectedWupin ? trendOf(snapshots, fuwuqiId, selectedWupin.id) : [];
  const serverSnapshots = useMemo(
    () => snapshots.filter((snapshot) => snapshot.fuwuqi_id === fuwuqiId),
    [snapshots, fuwuqiId],
  );

  const pasteResult = useMemo(() => parsePastePrices(pasteText, catalog.wupin), [pasteText, catalog.wupin]);
  const hasMarketPrice = useMemo(
    () => catalog.wupin.some((wupin) => getJiage(prices, fuwuqiId, wupin.id) !== undefined),
    [catalog.wupin, prices, fuwuqiId],
  );

  const openEdit = (wupin: Wupin, danjia: Tong | null) => {
    setEditingId(wupin.id);
    setEditValue(danjia ?? 0);
  };

  const commitEdit = () => {
    if (!editingId) return;
    if (editValue <= 0) {
      addToast("价格必须大于 0", "warning");
      return;
    }
    onOverridePrice(editingId, editValue);
    setEditingId(null);
  };

  const confirmPaste = () => {
    if (pasteResult.entries.length === 0) {
      addToast("没有可导入的价格", "warning");
      return;
    }
    onImportPrices(
      pasteResult.entries.map((entry) => ({ wupinId: entry.wupinId, danjia: entry.danjia })),
      "粘贴导入",
    );
    setPasteText("");
  };

  const exportJson = useCallback(() => {
    const jiage: JiageDaochuFile["jiage"] = [];
    for (const wupin of catalog.wupin) {
      const entry = getJiage(prices, fuwuqiId, wupin.id);
      if (!entry) continue;
      jiage.push({
        wupin_id: wupin.id,
        danjia: entry.danjia,
        gengxin_shijian: entry.gengxin_shijian,
        laiyuan: entry.laiyuan,
        shoudong: entry.shoudong,
      });
    }
    const file: JiageDaochuFile = {
      version: JIAGE_FILE_VERSION,
      fuwuqi_id: fuwuqiId,
      daochu_shijian: nowSeconds(),
      jiage,
    };
    downloadJson(`jxcg-hangqing-${fuwuqiId}-${dateStamp()}.json`, file);
    addToast(`已导出 ${jiage.length} 条行情`, "success");
    setStatus(`已导出 ${jiage.length} 条行情到 JSON`);
  }, [catalog.wupin, prices, fuwuqiId, addToast, setStatus]);

  const importJson = useCallback(async () => {
    const picked = await pickTextFile();
    if (!picked) return;
    try {
      const parsed: unknown = JSON.parse(picked.text);
      if (!isRecord(parsed) || !Array.isArray(parsed.jiage)) {
        throw new Error("JSON 结构应为 { version, fuwuqi_id, jiage: [...] }");
      }
      const version = Number(parsed.version ?? 0);
      if (version > JIAGE_FILE_VERSION) throw new Error(`文件版本 ${version} 高于当前应用支持的 ${JIAGE_FILE_VERSION}`);
      const entries: Array<{ wupinId: string; danjia: Tong }> = [];
      const errors: string[] = [];
      (parsed.jiage as unknown[]).forEach((raw, index) => {
        if (!isRecord(raw)) {
          errors.push(`第 ${index + 1} 条不是对象`);
          return;
        }
        const wupinId = raw.wupin_id;
        const danjia = Number(raw.danjia);
        if (typeof wupinId !== "string" || !Number.isFinite(danjia) || danjia <= 0) {
          errors.push(`第 ${index + 1} 条缺少 wupin_id 或单价非法`);
          return;
        }
        entries.push({ wupinId, danjia: Math.trunc(danjia) });
      });
      if (entries.length === 0) throw new Error("没有可导入的有效价格");
      onImportPrices(entries, picked.name);
      if (errors.length > 0) addToast(`导入完成，忽略 ${errors.length} 条异常记录`, "warning");
    } catch (err) {
      addToast(`导入 JSON 失败：${err instanceof Error ? err.message : String(err)}`, "error");
    }
  }, [onImportPrices, addToast]);

  const toggleFavorite = (wupinId: string) => onToggleFavorite(wupinFavKey(wupinId));

  return (
    <div className={common.page}>
      <div className={common.pageHeader}>
        <div>
          <h1 className={common.pageTitle}>行情</h1>
          <p className={common.pageSub}>
            {fuwuqiName} · 有价 {pricedCount}/{catalog.wupin.length} · 最近更新 {formatShijian(latestUpdate)} · 保管费{" "}
            {formatTong(shouxufei.baoguanfei)}/次
          </p>
        </div>
        <div className={common.toolbar}>
          <button className={common.btn} onClick={onSaveSnapshot} title="把当前价格存为快照，用于趋势图">
            <IconCamera size={14} />
            保存快照
          </button>
          <button className={common.btn} onClick={() => void importJson()} title="从 JSON 文件导入行情">
            <IconUpload size={14} />
            导入 JSON
          </button>
          <button className={common.btn} onClick={exportJson} title="导出当前服务器行情为 JSON">
            <IconDownload size={14} />
            导出 JSON
          </button>
          <button
            className={common.btnPrimary}
            onClick={onRefresh}
            disabled={refreshing}
            title={`联网刷新当前服全部相关物品（${refreshCount} 个，首次约 1–2 分钟；失败保留最近价格）`}
          >
            <IconRefresh size={14} />
            {refreshing ? "刷新中…" : `一键刷新（${refreshCount}）`}
          </button>
        </div>
      </div>

      {/* 首次启动/切换服务器：本地还没有行情档案 */}
      {!hasMarketPrice && !refreshing && (
        <div className={common.notice} role="status">
          <IconRefresh size={14} />
          <span>
            当前服务器还没有本地行情：点右上角「一键刷新」联网拉价（{refreshCount} 个物品，首次约 1–2 分钟；
            失败保留最近价格），或使用「粘贴导入 / 导入 JSON」录价。
          </span>
        </div>
      )}

      {/* 刷新进度 */}
      {refreshing && (
        <div className={common.notice} role="status" aria-live="polite">
          <IconRefresh size={14} />
          <span>
            正在拉取 {progress?.total ? `${progress.done}/${progress.total}` : "行情"}…
            {progress && progress.failed > 0 ? `（失败 ${progress.failed}）` : ""}
          </span>
          <span className={common.progressTrack}>
            <span
              className={common.progressFill}
              style={{ width: `${progress?.total ? Math.round((progress.done / progress.total) * 100) : 10}%` }}
            />
          </span>
        </div>
      )}

      {/* 工具条 + 粘贴导入 */}
      <section className={common.card}>
        <div className={styles.toolbarRow}>
          <input
            className={clsx(common.input, styles.search)}
            placeholder="搜索物品名 / id"
            value={prefs.search}
            onChange={(event) => setPrefs((previous) => ({ ...previous, search: event.target.value }))}
            aria-label="搜索物品"
          />
          <select
            className={clsx(common.select, styles.pinleiSelect)}
            value={prefs.pinlei}
            onChange={(event) =>
              setPrefs((previous) => ({ ...previous, pinlei: event.target.value as typeof previous.pinlei }))
            }
            aria-label="类别筛选"
          >
            <option value="all">全部类别</option>
            {PINLEI_LIST.map((pinlei) => (
              <option key={pinlei.id} value={pinlei.id}>
                {pinlei.label}
              </option>
            ))}
          </select>
          <label className={common.checkboxRow}>
            <input
              type="checkbox"
              checked={prefs.only_fav}
              onChange={(event) => setPrefs((previous) => ({ ...previous, only_fav: event.target.checked }))}
            />
            只看收藏
          </label>
          <span className={styles.toolbarSpacer} />
          <button
            className={common.btn}
            onClick={() => setPasteText((previous) => (previous === "" ? "5_30853 1金20银\n云芝 120金\n" : ""))}
            title="展开/收起批量粘贴导入"
          >
            粘贴导入
          </button>
          <span className={common.hint}>
            {rows.length > visibleRows.length
              ? `显示前 ${visibleRows.length} / 共 ${rows.length} 个物品`
              : `共 ${rows.length} 个物品`}
          </span>
        </div>
        {pasteText !== "" && (
          <div className={styles.pastePanel}>
            <textarea
              className={styles.textarea}
              value={pasteText}
              onChange={(event) => setPasteText(event.target.value)}
              placeholder={"每行：物品ID或名称 + 价格\n例如：\n5_30853 1金20银\n云芝 120金\n# 以 # 开头为注释"}
              aria-label="批量粘贴导入内容"
            />
            <div className={common.toolbar}>
              <span className={common.hint}>
                解析到 {pasteResult.entries.length} 条价格
                {pasteResult.errors.length > 0 ? `，${pasteResult.errors.length} 行失败` : ""}
              </span>
              <button className={common.btnPrimary} onClick={confirmPaste} disabled={pasteResult.entries.length === 0}>
                确认导入
              </button>
              <button className={common.btn} onClick={() => setPasteText("")}>
                清空
              </button>
            </div>
            {pasteResult.errors.length > 0 && (
              <div className={common.noticeDanger}>
                {pasteResult.errors.slice(0, 8).map((error) => (
                  <span key={error}>{error}</span>
                ))}
                {pasteResult.errors.length > 8 ? ` …共 ${pasteResult.errors.length} 行` : ""}
              </div>
            )}
          </div>
        )}
      </section>

      <div className={styles.layout}>
        {/* 价格表 */}
        <section className={clsx(common.card, styles.tableCard)}>
          <div className={common.cardTitle}>
            <span>物品价格</span>
            <span className={common.hint}>点行查看趋势 · 手填价优先于自动价</span>
          </div>
          <div className={common.tableWrap}>
            <table className={common.table}>
              <thead>
                <tr>
                  <th>物品</th>
                  <th>类别</th>
                  <th className={common.num}>单价（铜）</th>
                  <th>来源</th>
                  <th>更新时间</th>
                  <th className={common.center}>收藏</th>
                  <th className={common.center}>操作</th>
                </tr>
              </thead>
              <tbody>
                {rows.length === 0 && (
                  <tr>
                    <td colSpan={7}>
                      <p className={common.empty}>没有匹配的物品</p>
                    </td>
                  </tr>
                )}
                {visibleRows.map(({ wupin, entry, danjia, laiyuan }) => {
                  const favorite = favorites.includes(wupinFavKey(wupin.id));
                  return (
                    <tr
                      key={wupin.id}
                      className={clsx(common.rowButton, selectedId === wupin.id && styles.rowSelected)}
                      onClick={() => setPrefs((previous) => ({ ...previous, selected_wupin_id: wupin.id }))}
                    >
                      <td>
                        {wupin.mingcheng}
                        <span className={common.hint}> {wupin.id}</span>
                      </td>
                      <td>{PINLEI_LABEL[wupin.pinlei]}</td>
                      <td className={common.num}>
                        {editingId === wupin.id ? (
                          <MoneyInput
                            size="compact"
                            ariaLabel={`${wupin.mingcheng} 新单价`}
                            value={editValue}
                            onChange={(tong) => setEditValue(tong)}
                          />
                        ) : danjia === null ? (
                          <span className={common.badgeQuejia}>缺价</span>
                        ) : (
                          formatTong(danjia)
                        )}
                      </td>
                      <td>
                        <span className={LAIGUAN_BADGE[laiyuan] ?? common.badge}>{sourceLabel(laiyuan)}</span>
                      </td>
                      <td className={common.muted}>
                        {entry ? formatShijian(entry.gengxin_shijian) : "—"}
                      </td>
                      <td className={common.center}>
                        <button
                          className={favorite ? common.starOn : common.starBtn}
                          title={favorite ? "取消收藏" : "收藏"}
                          aria-label={favorite ? "取消收藏" : "收藏"}
                          onClick={(event) => {
                            event.stopPropagation();
                            toggleFavorite(wupin.id);
                          }}
                        >
                          <IconStar size={14} />
                        </button>
                      </td>
                      <td className={common.center}>
                        {editingId === wupin.id ? (
                          <span className={styles.rowActions}>
                            <button
                              className={common.btnGhost}
                              onClick={(event) => {
                                event.stopPropagation();
                                commitEdit();
                              }}
                            >
                              保存
                            </button>
                            <button
                              className={common.btnGhost}
                              onClick={(event) => {
                                event.stopPropagation();
                                setEditingId(null);
                              }}
                            >
                              取消
                            </button>
                          </span>
                        ) : (
                          <button
                            className={common.btnGhost}
                            onClick={(event) => {
                              event.stopPropagation();
                              openEdit(wupin, danjia);
                            }}
                          >
                            改价
                          </button>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </section>

        {/* 趋势 + 快照 */}
        <div className={styles.side}>
          <section className={common.card}>
            <div className={common.cardTitle}>
              <span>价格趋势</span>
              <span className={common.hint}>
                {selectedWupin ? `${selectedWupin.mingcheng}${selectedDisplay ? ` · 现价 ${formatTong(selectedDisplay.danjia)}` : ""}` : "点左表选物品"}
              </span>
            </div>
            {selectedWupin ? (
              trend.length > 0 ? (
                <div className={styles.chart}>
                  <ResponsiveContainer width="100%" height={190}>
                    <LineChart data={trend} margin={{ top: 6, right: 8, bottom: 0, left: 0 }}>
                      <CartesianGrid stroke="var(--border-subtle)" strokeDasharray="3 3" />
                      <XAxis
                        dataKey="shijian"
                        type="number"
                        domain={["dataMin", "dataMax"]}
                        tickFormatter={(value) => formatShijianShort(Number(value))}
                        tick={{ fontSize: 11, fill: "var(--text-muted)" }}
                      />
                      <YAxis
                        tickFormatter={(value) => formatTongCompact(Number(value))}
                        tick={{ fontSize: 11, fill: "var(--text-muted)" }}
                        width={64}
                      />
                      <Tooltip
                        formatter={(value) => formatTong(Number(value))}
                        labelFormatter={(value) => formatShijian(Number(value))}
                        contentStyle={{
                          background: "var(--bg-surface)",
                          border: "1px solid var(--border-default)",
                          borderRadius: 8,
                          fontSize: 12,
                        }}
                      />
                      <Line
                        type="monotone"
                        dataKey="danjia"
                        name="单价"
                        stroke="var(--primary-500)"
                        strokeWidth={2}
                        dot={{ r: 3 }}
                        activeDot={{ r: 4 }}
                      />
                    </LineChart>
                  </ResponsiveContainer>
                </div>
              ) : (
                <p className={common.empty}>暂无快照：刷新行情会自动保存快照，或点上方「保存快照」。</p>
              )
            ) : (
              <p className={common.empty}>点击价格表中的物品查看趋势。</p>
            )}
          </section>

          <section className={common.card}>
            <div className={common.cardTitle}>
              <span>历史快照</span>
              <span className={common.hint}>
                <IconClock size={12} /> {serverSnapshots.length} 条
              </span>
            </div>
            {serverSnapshots.length === 0 ? (
              <p className={common.empty}>还没有保存过快照。</p>
            ) : (
              <ul className={styles.snapshotList}>
                {[...serverSnapshots]
                  .sort((a, b) => b.shijian - a.shijian)
                  .slice(0, 12)
                  .map((snapshot) => (
                    <li key={snapshot.shijian} className={styles.snapshotItem}>
                      <span>{formatShijian(snapshot.shijian)}</span>
                      <span className={common.hint}>{Object.keys(snapshot.jiage).length} 个物品</span>
                      <button
                        className={common.btnGhost}
                        onClick={() => onDeleteSnapshot(snapshot.shijian)}
                        title="删除该快照"
                      >
                        删除
                      </button>
                    </li>
                  ))}
              </ul>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
