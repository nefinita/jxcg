import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api/commands";
import MoneyInput from "./MoneyInput";
import ResultCard, { type ResultItem } from "./ResultCard";
import { IconStar } from "./icons";
import clsx from "../utils/clsx";
import common from "../styles/common.module.css";
import styles from "./CalcPage.module.css";
import { usePersistentState } from "../hooks/usePersistentState";
import { calcOverridesSchema, calcPrefsSchema, DEFAULT_CALC_PREFS } from "../state/schemas";
import { JIYI_LABEL, JIYI_LIST, QUZHENG_LABEL, STORAGE_KEYS } from "../utils/constants";
import { formatBp, formatQiwang, clampInt } from "../utils/format";
import { formatTong } from "../utils/money";
import { jiageMapOf } from "../utils/jiage";
import type {
  Catalog,
  CailiaoMingxi,
  JiageBiao,
  JisuanResult,
  NetResult,
  Peifang,
  Settings,
  Shouxufei,
} from "../types";
import type { Toast } from "../hooks/useToast";

const LAIGUAN_LABEL: Record<string, string> = {
  hangqing: "行情",
  shangdian: "商店",
  shoudong: "手填",
  quejia: "缺价",
};

const LAIGUAN_BADGE: Record<string, string> = {
  hangqing: common.badgeHangqing,
  shangdian: common.badgeShangdian,
  shoudong: common.badgeShoudong,
  quejia: common.badgeQuejia,
};

export interface CalcPageProps {
  catalog: Catalog;
  settings: Settings;
  updateSettings: (patch: Partial<Settings>) => void;
  shouxufei: Shouxufei;
  prices: JiageBiao;
  fuwuqiId: string;
  favorites: string[];
  onToggleFavorite: (key: string) => void;
  /** 排行页点行跳转：非空时切到配方模式并选中该配方 */
  focusRecipe: { peifangId: string; nonce: number } | null;
  addToast: (message: string, type?: Toast["type"]) => void;
  setStatus: (message: string) => void;
}

/** 配方收藏 key（与物品收藏区分）。 */
export function peifangFavKey(id: string): string {
  return `peifang:${id}`;
}

export default function CalcPage({
  catalog,
  settings,
  updateSettings,
  shouxufei,
  prices,
  fuwuqiId,
  favorites,
  onToggleFavorite,
  focusRecipe,
  addToast,
  setStatus,
}: CalcPageProps) {
  const [prefs, setPrefs] = usePersistentState(STORAGE_KEYS.calcPrefs, () => DEFAULT_CALC_PREFS, calcPrefsSchema);
  const [overrides, setOverrides] = usePersistentState<Record<string, number>>(
    STORAGE_KEYS.calcOverrides,
    () => ({}),
    calcOverridesSchema,
  );
  const [editingWupin, setEditingWupin] = useState<string | null>(null);
  const [result, setResult] = useState<JisuanResult | null>(null);
  const [calculating, setCalculating] = useState(false);
  const [recipeError, setRecipeError] = useState("");
  const [net, setNet] = useState<NetResult | null>(null);
  const [baoben, setBaoben] = useState<number | null>(null);
  const [manualError, setManualError] = useState("");

  const recipeRev = useRef(0);
  const manualRev = useRef(0);

  // ---- 配方筛选与选中 ----
  const filteredPeifang = useMemo(() => {
    const query = prefs.search.trim().toLowerCase();
    return catalog.peifang.filter((peifang) => {
      if (query && !peifang.mingcheng.toLowerCase().includes(query) && !peifang.id.toLowerCase().includes(query)) {
        return false;
      }
      if (prefs.jiyi !== "all" && peifang.jiyi !== prefs.jiyi) return false;
      if (prefs.only_fav && !favorites.includes(peifangFavKey(peifang.id))) return false;
      return true;
    });
  }, [catalog.peifang, prefs.search, prefs.jiyi, prefs.only_fav, favorites]);

  // 优先保持当前选中；筛选把当前配方排除时，自动落到筛选结果的第一条。
  // 全量数据（~1000 配方）下只渲染前 120 条匹配项，避免切页卡顿。
  const visiblePeifang = useMemo(() => filteredPeifang.slice(0, 120), [filteredPeifang]);

  const selected: Peifang | undefined = useMemo(
    () =>
      filteredPeifang.find((peifang) => peifang.id === prefs.peifang_id) ??
      filteredPeifang[0] ??
      catalog.peifang.find((peifang) => peifang.id === prefs.peifang_id),
    [catalog.peifang, prefs.peifang_id, filteredPeifang],
  );

  useEffect(() => {
    if (selected && selected.id !== prefs.peifang_id) {
      setPrefs((previous) => ({ ...previous, peifang_id: selected.id }));
    }
  }, [selected, prefs.peifang_id, setPrefs]);

  // 排行页跳转：切到配方模式并清空筛选，确保选中配方可见。
  const focusNonce = focusRecipe?.nonce ?? 0;
  useEffect(() => {
    if (!focusRecipe) return;
    setPrefs((previous) => ({
      ...previous,
      mode: "recipe",
      peifang_id: focusRecipe.peifangId,
      search: "",
      jiyi: "all",
      only_fav: false,
    }));
    // 只在 nonce 变化时触发，避免同一次跳转被 setPrefs 反复覆盖。
  }, [focusNonce]);

  // ---- 请求价格映射（行情 > 商店价由后端兜底；手改覆盖优先） ----
  const priceMap = useMemo(() => {
    if (!selected) return {};
    const ids = [
      ...selected.cailiao.map((item) => item.wupin_id),
      ...selected.chuchan.map((slot) => slot.wupin_id),
    ];
    const map = jiageMapOf(prices, fuwuqiId, ids);
    for (const id of selected.cailiao.map((item) => item.wupin_id)) {
      const override = overrides[id];
      if (typeof override === "number" && override > 0) map[id] = override;
    }
    return map;
  }, [selected, prices, fuwuqiId, overrides]);

  // ---- 配方模式：自动重算（防抖 + 版本号防迟到响应） ----
  useEffect(() => {
    if (prefs.mode !== "recipe" || !selected) {
      setResult(null);
      setCalculating(false);
      return;
    }
    const rev = ++recipeRev.current;
    setCalculating(true);
    setRecipeError("");
    const timer = window.setTimeout(() => {
      api
        .calculateRecipe({
          peifang_id: selected.id,
          cishu: Math.max(1, prefs.cishu),
          guadan_cishu: Math.max(1, prefs.guadan_cishu),
          jingli_danjia: settings.jingli_danjia,
          shouxufei,
          jiage: priceMap,
          jiyi_dengji: settings.jiyi_dengji,
        })
        .then((data) => {
          if (rev !== recipeRev.current) return;
          setResult(data);
          setCalculating(false);
        })
        .catch((err: unknown) => {
          if (rev !== recipeRev.current) return;
          setRecipeError(err instanceof Error ? err.message : String(err));
          setCalculating(false);
        });
    }, 160);
    return () => {
      window.clearTimeout(timer);
      ++recipeRev.current;
    };
  }, [
    prefs.mode,
    selected,
    prefs.cishu,
    prefs.guadan_cishu,
    settings.jingli_danjia,
    settings.jiyi_dengji,
    shouxufei,
    priceMap,
  ]);

  // ---- 手动模式：净收入 + 保本价 ----
  useEffect(() => {
    if (prefs.mode !== "manual") return;
    const rev = ++manualRev.current;
    const timer = window.setTimeout(() => {
      Promise.all([
        api.calculateNet({
          shoujia: prefs.shoujia,
          shuliang: Math.max(1, prefs.shuliang),
          shouxufei,
          guadan_cishu: Math.max(1, prefs.guadan_cishu),
        }),
        api.calculateBaoben({
          chengben: prefs.chengben,
          shouxufei,
          guadan_cishu: Math.max(1, prefs.guadan_cishu),
        }),
      ])
        .then(([netResult, baobenResult]) => {
          if (rev !== manualRev.current) return;
          setNet(netResult);
          setBaoben(baobenResult);
          setManualError("");
        })
        .catch((err: unknown) => {
          if (rev !== manualRev.current) return;
          setManualError(err instanceof Error ? err.message : String(err));
        });
    }, 160);
    return () => {
      window.clearTimeout(timer);
      ++manualRev.current;
    };
  }, [prefs.mode, prefs.shoujia, prefs.shuliang, prefs.guadan_cishu, prefs.chengben, shouxufei]);

  const handleOverride = useCallback(
    (wupinId: string, tong: number) => {
      setOverrides((previous) => ({ ...previous, [wupinId]: tong }));
      setEditingWupin(null);
      setStatus(`已手改材料单价：${formatTong(tong)}`);
      addToast(`材料单价已改为 ${formatTong(tong)}`, "success");
    },
    [setOverrides, setStatus, addToast],
  );

  const clearOverride = useCallback(
    (wupinId: string) => {
      setOverrides((previous) => {
        const next = { ...previous };
        delete next[wupinId];
        return next;
      });
      setEditingWupin(null);
    },
    [setOverrides],
  );

  const toggleMode = (mode: "recipe" | "manual") => {
    setPrefs((previous) => ({ ...previous, mode }));
  };

  // ---- 材料行（优先用后端结果；无结果时用本地数据先渲染） ----
  const cailiaoRows: CailiaoMingxi[] = useMemo(() => {
    if (result) {
      return result.cailiao.map((row) => ({
        ...row,
        laiyuan: overrides[row.wupin_id] ? "shoudong" : row.laiyuan,
      }));
    }
    if (!selected) return [];
    return selected.cailiao.map((item) => {
      const wupin = catalog.wupin.find((entry) => entry.id === item.wupin_id);
      const override = overrides[item.wupin_id];
      const market = priceMap[item.wupin_id];
      const danjia = override ?? market ?? wupin?.shangdianjia ?? 0;
      const laiyuan = override ? "shoudong" : market ? "hangqing" : wupin?.shangdianjia ? "shangdian" : "quejia";
      return {
        wupin_id: item.wupin_id,
        mingcheng: wupin?.mingcheng ?? item.wupin_id,
        shuliang: item.shuliang,
        danjia,
        xiaojia: danjia * item.shuliang * Math.max(1, prefs.cishu),
        laiyuan,
      };
    });
  }, [result, selected, catalog.wupin, overrides, priceMap, prefs.cishu]);

  const quejiaNames = (result?.quejia ?? []).map(
    (id) => catalog.wupin.find((wupin) => wupin.id === id)?.mingcheng ?? id,
  );

  const resultItems: ResultItem[] = result
    ? [
        {
          label: "净利润",
          value: result.lirun,
          kind: "money",
          emphasis: true,
          tone: result.lirun < 0 ? "negative" : "positive",
        },
        { label: "总成本", value: result.zong_chengben, kind: "money" },
        { label: "税后收入", value: result.jingshouru, kind: "money" },
        { label: "利润率", value: result.lirunlv_bp, kind: "bp", tone: result.lirunlv_bp < 0 ? "negative" : "default" },
        { label: "每精力利润", value: result.mei_jingli, kind: "money" },
        { label: "保本价", value: result.baobenjia, kind: "money", hint: "按期望产出折算的单价" },
        { label: "期望产出", value: result.chuchan_qiwang, kind: "number", hint: "件" },
        { label: "精力消耗", value: result.jingli, kind: "number" },
      ]
    : [];

  const manualItems: ResultItem[] = net
    ? [
        { label: "到手", value: net.daoshou, kind: "money", emphasis: true, tone: net.daoshou < 0 ? "negative" : "positive" },
        { label: "成交总额", value: net.zonge, kind: "money" },
        { label: "手续费", value: net.feiyong, kind: "money" },
        { label: "保管费", value: net.baoguanfei, kind: "money" },
      ]
    : [];

  const baobenItems: ResultItem[] =
    baoben === null
      ? []
      : [
          { label: "保本总售价", value: baoben, kind: "money", emphasis: true, hint: "到手 ≥ 成本的最小售价" },
          { label: "成本", value: prefs.chengben, kind: "money" },
          {
            label: "保本单价",
            value: Math.ceil(baoben / Math.max(1, prefs.shuliang)),
            kind: "money",
            hint: `按 ${Math.max(1, prefs.shuliang)} 件折算`,
          },
        ];

  return (
    <div className={common.page}>
      <div className={common.pageHeader}>
        <div>
          <h1 className={common.pageTitle}>计算</h1>
          <p className={common.pageSub}>
            交易行手续费 {formatBp(shouxufei.shuilv_bp)}（{QUZHENG_LABEL[shouxufei.quzheng]}）+ 保管费{" "}
            {formatTong(shouxufei.baoguanfei)}/次
          </p>
        </div>
        <div className={common.segmented} role="group" aria-label="计算模式">
          <button
            className={clsx(common.segmentedBtn, prefs.mode === "recipe" && common.segmentedActive)}
            aria-pressed={prefs.mode === "recipe"}
            onClick={() => toggleMode("recipe")}
          >
            配方模式
          </button>
          <button
            className={clsx(common.segmentedBtn, prefs.mode === "manual" && common.segmentedActive)}
            aria-pressed={prefs.mode === "manual"}
            onClick={() => toggleMode("manual")}
          >
            手动模式
          </button>
        </div>
      </div>

      {prefs.mode === "manual" ? (
        <div className={common.grid2}>
          <section className={common.card}>
            <div className={common.cardTitle}>交易行净收入</div>
            <div className={styles.formStack}>
              <label className={common.field}>
                <span className={common.fieldLabel}>售价（每件）</span>
                <MoneyInput
                  ariaLabel="售价"
                  value={prefs.shoujia}
                  onChange={(tong) => setPrefs((previous) => ({ ...previous, shoujia: tong }))}
                />
              </label>
              <div className={styles.paramGrid}>
                <label className={common.field}>
                  <span className={common.fieldLabel}>挂单数量</span>
                  <input
                    className={common.numberInput}
                    type="number"
                    min={1}
                    value={prefs.shuliang}
                    onChange={(event) =>
                      setPrefs((previous) => ({ ...previous, shuliang: clampInt(Number(event.target.value), 1, 999) }))
                    }
                  />
                </label>
                <label className={common.field}>
                  <span className={common.fieldLabel}>挂单次数（保管费按次）</span>
                  <input
                    className={common.numberInput}
                    type="number"
                    min={1}
                    value={prefs.guadan_cishu}
                    onChange={(event) =>
                      setPrefs((previous) => ({
                        ...previous,
                        guadan_cishu: clampInt(Number(event.target.value), 1, 99),
                      }))
                    }
                  />
                </label>
              </div>
              <div className={styles.paramDivider} />
              <label className={common.field}>
                <span className={common.fieldLabel}>保本计算用的总成本</span>
                <MoneyInput
                  ariaLabel="总成本"
                  value={prefs.chengben}
                  onChange={(tong) => setPrefs((previous) => ({ ...previous, chengben: tong }))}
                />
              </label>
              {manualError && <div className={common.errorText}>{manualError}</div>}
              <p className={common.hint}>
                净收入 = 售价 × 数量 − 手续费（{formatBp(shouxufei.shuilv_bp)}，{QUZHENG_LABEL[shouxufei.quzheng]}）− 保管费 ×
                挂单次数；保本价按「到手 ≥ 成本」逐铜求解。
              </p>
            </div>
          </section>
          <div className={styles.resultStack}>
            <ResultCard
              title="到手金额"
              items={manualItems}
              emptyText="计算中…"
              footer={<span className={common.hint}>数量 {Math.max(1, prefs.shuliang)} 件</span>}
            />
            <ResultCard title="保本价" items={baobenItems} emptyText="计算中…" />
          </div>
        </div>
      ) : (
        <div className={common.grid3}>
          {/* 左：服务器 / 配方 / 参数 */}
          <section className={common.card}>
            <div className={common.cardTitle}>
              <span>配方与参数</span>
              <span className={common.hint}>
                {filteredPeifang.length > visiblePeifang.length
                  ? `显示前 ${visiblePeifang.length} / 共 ${filteredPeifang.length} 个`
                  : `${filteredPeifang.length} 个配方`}
              </span>
            </div>
            <div className={styles.formStack}>
              <label className={common.field}>
                <span className={common.fieldLabel}>服务器（行情来源）</span>
                <select
                  className={common.select}
                  value={settings.fuwuqi_id}
                  onChange={(event) => updateSettings({ fuwuqi_id: event.target.value })}
                >
                  {catalog.fuwuqi.map((fuwuqi) => (
                    <option key={fuwuqi.id} value={fuwuqi.id}>
                      {fuwuqi.mingcheng}
                      {fuwuqi.daju ? ` · ${fuwuqi.daju}` : ""}
                    </option>
                  ))}
                </select>
              </label>

              <div className={common.fieldRow}>
                <input
                  className={common.input}
                  placeholder="搜索配方名 / id"
                  value={prefs.search}
                  onChange={(event) => setPrefs((previous) => ({ ...previous, search: event.target.value }))}
                  aria-label="搜索配方"
                />
              </div>
              <div className={common.toolbar}>
                <button
                  className={clsx(common.chip, prefs.jiyi === "all" && common.chipActive)}
                  onClick={() => setPrefs((previous) => ({ ...previous, jiyi: "all" }))}
                >
                  全部
                </button>
                {JIYI_LIST.map((jiyi) => (
                  <button
                    key={jiyi.id}
                    className={clsx(common.chip, prefs.jiyi === jiyi.id && common.chipActive)}
                    onClick={() => setPrefs((previous) => ({ ...previous, jiyi: jiyi.id }))}
                  >
                    {jiyi.label}
                  </button>
                ))}
                <label className={common.checkboxRow}>
                  <input
                    type="checkbox"
                    checked={prefs.only_fav}
                    onChange={(event) =>
                      setPrefs((previous) => ({ ...previous, only_fav: event.target.checked }))
                    }
                  />
                  只看收藏
                </label>
              </div>

              <div className={common.scrollList} role="listbox" aria-label="配方列表">
                {filteredPeifang.length === 0 && <p className={common.empty}>没有匹配的配方</p>}
                {visiblePeifang.map((peifang) => {
                  const favorite = favorites.includes(peifangFavKey(peifang.id));
                  return (
                    <div
                      key={peifang.id}
                      className={clsx(common.listItem, selected?.id === peifang.id && common.listItemActive)}
                      role="option"
                      aria-selected={selected?.id === peifang.id}
                      onClick={() => setPrefs((previous) => ({ ...previous, peifang_id: peifang.id }))}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          setPrefs((previous) => ({ ...previous, peifang_id: peifang.id }));
                        }
                      }}
                      tabIndex={0}
                    >
                      <button
                        className={favorite ? common.starOn : common.starBtn}
                        title={favorite ? "取消收藏" : "收藏配方"}
                        aria-label={favorite ? "取消收藏" : "收藏配方"}
                        onClick={(event) => {
                          event.stopPropagation();
                          onToggleFavorite(peifangFavKey(peifang.id));
                        }}
                      >
                        <IconStar size={14} />
                      </button>
                      <span className={styles.recipeName}>{peifang.mingcheng}</span>
                      <span className={common.listItemMeta}>
                        {JIYI_LABEL[peifang.jiyi]} · 精力 {peifang.jingli}
                      </span>
                    </div>
                  );
                })}
              </div>

              <div className={styles.paramDivider} />
              <div className={styles.paramGrid}>
                <label className={common.field}>
                  <span className={common.fieldLabel}>制作次数</span>
                  <input
                    className={common.numberInput}
                    type="number"
                    min={1}
                    value={prefs.cishu}
                    onChange={(event) =>
                      setPrefs((previous) => ({ ...previous, cishu: clampInt(Number(event.target.value), 1, 999) }))
                    }
                  />
                </label>
                <label className={common.field}>
                  <span className={common.fieldLabel}>挂单次数</span>
                  <input
                    className={common.numberInput}
                    type="number"
                    min={1}
                    value={prefs.guadan_cishu}
                    onChange={(event) =>
                      setPrefs((previous) => ({
                        ...previous,
                        guadan_cishu: clampInt(Number(event.target.value), 1, 99),
                      }))
                    }
                  />
                </label>
              </div>
              <label className={common.field}>
                <span className={common.fieldLabel}>精力单价（每点精力折合铜）</span>
                <MoneyInput
                  ariaLabel="精力单价"
                  value={settings.jingli_danjia}
                  onChange={(tong) => updateSettings({ jingli_danjia: tong })}
                />
              </label>
              <label className={common.field}>
                <span className={common.fieldLabel}>技艺等级（0–50）</span>
                <div className={common.fieldRow}>
                  <input
                    className={styles.range}
                    type="range"
                    min={0}
                    max={50}
                    value={settings.jiyi_dengji}
                    onChange={(event) => updateSettings({ jiyi_dengji: clampInt(Number(event.target.value), 0, 50) })}
                    aria-label="技艺等级"
                  />
                  <input
                    className={common.numberInput}
                    type="number"
                    min={0}
                    max={50}
                    value={settings.jiyi_dengji}
                    onChange={(event) =>
                      updateSettings({ jiyi_dengji: clampInt(Number(event.target.value), 0, 50) })
                    }
                  />
                </div>
              </label>
            </div>
          </section>

          {/* 中：材料表 + 产物 */}
          <section className={common.card}>
            <div className={common.cardTitle}>
              <span>材料表</span>
              <span className={common.hint}>
                {calculating ? "计算中…" : result ? `共 ${result.cailiao.length} 项材料` : ""}
              </span>
            </div>
            {recipeError && <div className={common.noticeDanger}>{recipeError}</div>}
            {cailiaoRows.length === 0 ? (
              <p className={common.empty}>请选择一个配方</p>
            ) : (
              <div className={common.tableWrap}>
                <table className={common.table}>
                  <thead>
                    <tr>
                      <th>材料</th>
                      <th>来源</th>
                      <th className={common.num}>单次需求</th>
                      <th className={common.num}>单价（铜）</th>
                      <th className={common.num}>小计</th>
                      <th className={common.center}>操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {cailiaoRows.map((row) => {
                      const overridden = overrides[row.wupin_id] !== undefined;
                      return (
                        <tr key={row.wupin_id}>
                          <td>{row.mingcheng}</td>
                          <td>
                            <span className={LAIGUAN_BADGE[row.laiyuan] ?? common.badge}>
                              {LAIGUAN_LABEL[row.laiyuan] ?? row.laiyuan}
                            </span>
                          </td>
                          <td className={common.num}>{row.shuliang}</td>
                          <td className={common.num}>
                            {editingWupin === row.wupin_id ? (
                              <MoneyInput
                                size="compact"
                                ariaLabel={`${row.mingcheng} 单价`}
                                value={row.danjia}
                                onChange={(tong) => handleOverride(row.wupin_id, tong)}
                              />
                            ) : (
                              formatTong(row.danjia)
                            )}
                          </td>
                          <td className={common.num}>{formatTong(row.xiaojia)}</td>
                          <td className={common.center}>
                            {overridden ? (
                              <button className={common.btnGhost} onClick={() => clearOverride(row.wupin_id)}>
                                恢复
                              </button>
                            ) : (
                              <button
                                className={common.btnGhost}
                                onClick={() => setEditingWupin(row.wupin_id)}
                                title="手动覆盖该材料单价（优先于行情）"
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
            )}
            {quejiaNames.length > 0 && (
              <div className={common.noticeDanger} role="alert">
                缺价：{quejiaNames.join("、")}（结果不完整，可在行情页录价或在此手改）
              </div>
            )}
            <div className={common.divider} />
            <div className={common.cardTitleSmall}>期望产出</div>
            {result && result.chuchan.length > 0 ? (
              <table className={common.table}>
                <thead>
                  <tr>
                    <th>产物</th>
                    <th className={common.num}>期望件数</th>
                    <th className={common.num}>单价（铜）</th>
                  </tr>
                </thead>
                <tbody>
                  {result.chuchan.map((row) => (
                    <tr key={row.wupin_id}>
                      <td>{row.mingcheng}</td>
                      <td className={common.num}>{formatQiwang(row.qiwang)}</td>
                      <td className={common.num}>{formatTong(row.danjia)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <p className={common.hint}>计算完成后展示期望产出明细。</p>
            )}
          </section>

          {/* 右：结果 */}
          <div className={styles.resultStack}>
            <ResultCard
              title={selected ? `${selected.mingcheng}${Math.max(1, prefs.cishu) > 1 ? ` × ${Math.max(1, prefs.cishu)}` : ""}` : "结果"}
              items={resultItems}
              emptyText={calculating ? "计算中…" : "请选择一个配方"}
              extra={
                result && (
                  <span className={result.lirun >= 0 ? common.badgeHangqing : common.badgeQuejia}>
                    {result.lirun >= 0 ? "可做" : "亏损"}
                  </span>
                )
              }
              footer={
                result && (
                  <>
                    <span className={common.hint}>
                      材料 {formatTong(result.chengben_cailiao)} · 精力 {formatTong(result.chengben_jingli)} · 制作费{" "}
                      {formatTong(result.zhizuofei)} · 保管费 {formatTong(result.baoguanfei)}
                    </span>
                    <span className={common.hint}>
                      值集：手续费 {formatBp(shouxufei.shuilv_bp)} · 保管费 {formatTong(shouxufei.baoguanfei)}/次
                    </span>
                  </>
                )
              }
            />
          </div>
        </div>
      )}
    </div>
  );
}
