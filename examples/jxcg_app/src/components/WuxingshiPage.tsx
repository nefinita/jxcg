import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api/commands";
import MoneyInput from "./MoneyInput";
import ResultCard, { type ResultItem } from "./ResultCard";
import clsx from "../utils/clsx";
import common from "../styles/common.module.css";
import styles from "./WuxingshiPage.module.css";
import { usePersistentState } from "../hooks/usePersistentState";
import { calcOverridesSchema, DEFAULT_WUXINGSHI_PREFS, wuxingshiPrefsSchema } from "../state/schemas";
import { STORAGE_KEYS } from "../utils/constants";
import { clampInt, formatBp, formatQiwang } from "../utils/format";
import { formatTong } from "../utils/money";
import { getJiage } from "../utils/jiage";
import type {
  Catalog,
  JiageBiao,
  Settings,
  Shouxufei,
  Tong,
  Zhuanhuan,
  ZhuanhuanResult,
} from "../types";

export interface WuxingshiPageProps {
  catalog: Catalog;
  settings: Settings;
  updateSettings: (patch: Partial<Settings>) => void;
  shouxufei: Shouxufei;
  prices: JiageBiao;
  fuwuqiId: string;
  fuwuqiName: string;
  /** 跳转行情页（刷新石头价格） */
  onOpenMarket: () => void;
  setStatus: (message: string) => void;
}

interface StonePrice {
  /** 生效单价（铜）；null = 缺价 */
  danjia: Tong | null;
  /** hangqing / shoudong / quejia */
  laiyuan: string;
  /** 是否用户手改 */
  override: boolean;
}

/** 石头单价：手填 > 行情缓存 > 缺价。 */
function stonePriceOf(
  wupinId: string | undefined,
  overrides: Record<string, Tong>,
  prices: JiageBiao,
  fuwuqiId: string,
): StonePrice {
  if (!wupinId) return { danjia: null, laiyuan: "quejia", override: false };
  const override = overrides[wupinId];
  if (typeof override === "number" && override > 0) {
    return { danjia: override, laiyuan: "shoudong", override: true };
  }
  const entry = getJiage(prices, fuwuqiId, wupinId);
  if (entry && entry.danjia > 0) {
    return { danjia: entry.danjia, laiyuan: entry.shoudong ? "shoudong" : "hangqing", override: false };
  }
  return { danjia: null, laiyuan: "quejia", override: false };
}

const LAIGUAN_LABEL: Record<string, string> = {
  hangqing: "行情",
  shoudong: "手填",
  quejia: "缺价",
};

const LAIGUAN_BADGE: Record<string, string> = {
  hangqing: common.badgeHangqing,
  shoudong: common.badgeShoudong,
  quejia: common.badgeQuejia,
};

/** 五行石精炼：cishu = 目标成功次数，期望投入 = cishu / p，成本按期望投入折算。 */
export default function WuxingshiPage({
  catalog,
  settings,
  updateSettings,
  shouxufei,
  prices,
  fuwuqiId,
  fuwuqiName,
  onOpenMarket,
  setStatus,
}: WuxingshiPageProps) {
  const [prefs, setPrefs] = usePersistentState(STORAGE_KEYS.wuxingshiPrefs, () => DEFAULT_WUXINGSHI_PREFS, wuxingshiPrefsSchema);
  const [overrides, setOverrides] = usePersistentState<Record<string, Tong>>(
    STORAGE_KEYS.wuxingshiOverrides,
    () => ({}),
    calcOverridesSchema,
  );
  const [result, setResult] = useState<ZhuanhuanResult | null>(null);
  const [error, setError] = useState("");
  const [calculating, setCalculating] = useState(false);
  const rev = useRef(0);

  const conversions: Zhuanhuan[] = catalog.wuxingshi;
  const selected = conversions.find((item) => item.id === prefs.zhuanhuan_id) ?? conversions[0];

  useEffect(() => {
    if (selected && selected.id !== prefs.zhuanhuan_id) {
      setPrefs((previous) => ({ ...previous, zhuanhuan_id: selected.id }));
    }
  }, [selected, prefs.zhuanhuan_id, setPrefs]);

  const wupinName = (id: string | undefined): string => {
    if (!id) return "—";
    return catalog.wupin.find((wupin) => wupin.id === id)?.mingcheng ?? id;
  };

  const congInfo = useMemo(
    () => stonePriceOf(selected?.cong, overrides, prices, fuwuqiId),
    [selected?.cong, overrides, prices, fuwuqiId],
  );
  const daoInfo = useMemo(
    () => stonePriceOf(selected?.dao, overrides, prices, fuwuqiId),
    [selected?.dao, overrides, prices, fuwuqiId],
  );

  const jiage = useMemo(() => {
    const map: Record<string, Tong> = {};
    if (selected && congInfo.danjia !== null) map[selected.cong] = congInfo.danjia;
    if (selected && daoInfo.danjia !== null) map[selected.dao] = daoInfo.danjia;
    return map;
  }, [selected, congInfo.danjia, daoInfo.danjia]);

  const cishu = Math.max(1, prefs.cishu);
  const chenggonglvBp = Math.min(10_000, Math.max(1, selected?.chenggonglv_bp ?? 10_000));
  const qiwangCishu = cishu / (chenggonglvBp / 10_000);
  const yiciChengben =
    (congInfo.danjia ?? 0) * (selected?.xuyao ?? 0) +
    (selected?.jinbi ?? 0) +
    (selected?.jingli ?? 0) * settings.jingli_danjia;

  // 目标次数/精力单价/石价变化 → 自动重算（mock 与 Tauri 同一条命令）。
  useEffect(() => {
    if (!selected) {
      setResult(null);
      setCalculating(false);
      return;
    }
    const currentRev = ++rev.current;
    setCalculating(true);
    setError("");
    const timer = window.setTimeout(() => {
      api
        .calculateConvert({
          zhuanhuan_id: selected.id,
          cishu: Math.max(1, prefs.cishu),
          jingli_danjia: settings.jingli_danjia,
          shouxufei,
          jiage,
        })
        .then((data) => {
          if (currentRev !== rev.current) return;
          setResult(data);
          setCalculating(false);
        })
        .catch((err: unknown) => {
          if (currentRev !== rev.current) return;
          setError(err instanceof Error ? err.message : String(err));
          setCalculating(false);
        });
    }, 160);
    return () => {
      window.clearTimeout(timer);
      ++rev.current;
    };
  }, [selected, prefs.cishu, settings.jingli_danjia, shouxufei, jiage]);

  const setStonePrice = (wupinId: string | undefined, tong: Tong) => {
    if (!wupinId) return;
    setOverrides((previous) => {
      const next = { ...previous };
      if (tong > 0) next[wupinId] = tong;
      else delete next[wupinId];
      return next;
    });
    if (tong > 0) setStatus(`已手改石头单价：${formatTong(tong)}`);
  };

  const clearStonePrice = (wupinId: string | undefined) => {
    if (!wupinId) return;
    setOverrides((previous) => {
      const next = { ...previous };
      delete next[wupinId];
      return next;
    });
    setStatus("已恢复行情价");
  };

  const quejiaNames = (result?.quejia ?? []).map((id) => wupinName(id));

  const resultItems: ResultItem[] = result
    ? [
        {
          label: "净利润",
          value: result.lirun,
          kind: "money",
          emphasis: true,
          tone: result.lirun < 0 ? "negative" : "positive",
          hint: `${cishu} 次成功 · 期望投入 ${formatQiwang(result.qiwang_cishu)} 次`,
        },
        { label: "利润率", value: result.lirunlv_bp, kind: "bp", tone: result.lirunlv_bp < 0 ? "negative" : "default" },
        { label: "期望投入次数", value: result.qiwang_cishu, kind: "number", hint: `${cishu} ÷ ${formatBp(chenggonglvBp)}` },
        { label: "期望总成本", value: result.zong_chengben, kind: "money" },
        { label: "期望收入", value: result.shouru, kind: "money", hint: "扣手续费前" },
        { label: "目标成功次数", value: cishu, kind: "number", hint: "= 期望产出件数" },
      ]
    : [];

  if (conversions.length === 0) {
    return (
      <div className={common.page}>
        <div className={common.pageHeader}>
          <h1 className={common.pageTitle}>五行石</h1>
        </div>
        <p className={common.empty}>随包数据还没有五行石转换配方（data/wuxingshi.toml 为空）。</p>
      </div>
    );
  }

  return (
    <div className={common.page}>
      <div className={common.pageHeader}>
        <div>
          <h1 className={common.pageTitle}>五行石</h1>
          <p className={common.pageSub}>
            低阶石 + 精力 + 金币 → 高阶石：目标成功次数 ÷ 成功率 = 期望投入次数，成本按期望投入折算
          </p>
        </div>
        <div className={common.toolbar}>
          <span className={common.hint}>
            {fuwuqiName} · 手续费 {formatBp(shouxufei.shuilv_bp)}（{selected?.laiyuan === "shequ-2023" ? "社区口径" : "随包数据"}）
          </span>
          <button className={common.btn} onClick={onOpenMarket} title="去行情页刷新两种五行石的价格">
            去行情页刷新价格
          </button>
        </div>
      </div>

      <div className={common.grid3}>
        {/* 左：转换配方 + 参数 */}
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>转换配方</span>
            <span className={common.hint}>{conversions.length} 条</span>
          </div>
          <div className={common.scrollList} role="listbox" aria-label="五行石转换配方">
            {conversions.map((item) => (
              <button
                key={item.id}
                className={clsx(common.listItem, selected?.id === item.id && common.listItemActive)}
                role="option"
                aria-selected={selected?.id === item.id}
                onClick={() => setPrefs((previous) => ({ ...previous, zhuanhuan_id: item.id }))}
              >
                <span className={styles.recipeName}>{item.mingcheng}</span>
                <span className={common.listItemMeta}>
                  投入 {item.xuyao} · 精力 {item.jingli} · {formatBp(item.chenggonglv_bp)}
                </span>
              </button>
            ))}
          </div>

          <div className={common.divider} />
          <div className={styles.form}>
            <label className={common.field}>
              <span className={common.fieldLabel}>目标成功次数（cishu）</span>
              <input
                className={common.numberInput}
                type="number"
                min={1}
                max={999}
                value={prefs.cishu}
                onChange={(event) =>
                  setPrefs((previous) => ({ ...previous, cishu: clampInt(Number(event.target.value), 1, 999) }))
                }
                aria-label="目标成功次数"
              />
            </label>
            <label className={common.field}>
              <span className={common.fieldLabel}>精力单价（每点精力折合铜）</span>
              <MoneyInput
                ariaLabel="精力单价"
                value={settings.jingli_danjia}
                onChange={(tong) => updateSettings({ jingli_danjia: tong })}
              />
              <span className={common.hint}>与计算页/设置页共用同一参数。</span>
            </label>
            <p className={common.hint}>
              {selected ? `单次投入：${selected.xuyao} × ${wupinName(selected.cong)} + ${formatTong(selected.jinbi)} 金币 + ${selected.jingli} 精力` : ""}
            </p>
            <p className={common.hint}>
              期望投入 ≈ {formatQiwang(qiwangCishu)} 次（{cishu} ÷ {formatBp(chenggonglvBp)}）
            </p>
          </div>
        </section>

        {/* 中：石头价格 + 配方详情 + 口径 */}
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>石头价格</span>
            <span className={common.hint}>行情库 · {fuwuqiName}</span>
          </div>
          <div className={styles.form}>
            {[
              { role: "低阶石", wupinId: selected?.cong, info: congInfo, ariaLabel: "低阶石单价" },
              { role: "高阶石", wupinId: selected?.dao, info: daoInfo, ariaLabel: "高阶石单价" },
            ].map(({ role, wupinId, info, ariaLabel }) => (
              <div key={role} className={styles.stoneRow}>
                <div className={styles.stoneHead}>
                  <span className={styles.stoneName}>
                    {role}：{wupinName(wupinId)}
                    {wupinId && <span className={common.hint}> {wupinId}</span>}
                  </span>
                  {info.danjia === null ? (
                    <span className={common.badgeQuejia}>缺价</span>
                  ) : (
                    <span className={LAIGUAN_BADGE[info.laiyuan] ?? common.badge}>{LAIGUAN_LABEL[info.laiyuan] ?? info.laiyuan}</span>
                  )}
                </div>
                <div className={styles.stoneBody}>
                  <MoneyInput
                    ariaLabel={ariaLabel}
                    value={info.danjia ?? 0}
                    onChange={(tong) => setStonePrice(wupinId, tong)}
                  />
                  {info.override && (
                    <button className={common.btn} onClick={() => clearStonePrice(wupinId)} title="清除手填价，恢复行情价">
                      恢复行情价
                    </button>
                  )}
                </div>
              </div>
            ))}
            <p className={common.hint}>
              缺价时按 0 参与计算并标红；手填价只对当前浏览器生效，行情价可通过行情页「一键刷新」更新。
            </p>
          </div>

          <div className={common.divider} />
          <div className={common.cardTitleSmall}>配方详情</div>
          <dl className={styles.detailList}>
            <div>
              <dt>成功率</dt>
              <dd>
                {formatBp(chenggonglvBp)}（{chenggonglvBp} bp）
              </dd>
            </div>
            <div>
              <dt>投入</dt>
              <dd>
                {selected?.xuyao ?? 0} 个 {wupinName(selected?.cong)}
              </dd>
            </div>
            <div>
              <dt>金币消耗</dt>
              <dd>{formatTong(selected?.jinbi ?? 0)}</dd>
            </div>
            <div>
              <dt>精力消耗</dt>
              <dd>{selected?.jingli ?? 0} 点</dd>
            </div>
            <div>
              <dt>单次投入成本</dt>
              <dd>{formatTong(yiciChengben)}</dd>
            </div>
            <div>
              <dt>数据来源</dt>
              <dd>{selected?.laiyuan ?? "随包数据"}</dd>
            </div>
          </dl>
          <p className={common.hint}>
            口径：cishu = 目标成功次数；期望投入次数 = cishu ÷ 成功率（几何分布）；期望成本 = 单次投入成本 × 期望投入次数；
            期望收入 = 目标成功次数 × 高阶石单价；净利润 = 扣手续费后的期望收入 − 期望成本（不含保管费）。
          </p>
        </section>

        {/* 右：结果 */}
        <div className={styles.resultStack}>
          <ResultCard
            title={selected ? `精炼结果 · ${selected.mingcheng}` : "精炼结果"}
            items={resultItems}
            emptyText={calculating ? "计算中…" : "请选择转换配方"}
            extra={
              result && (
                <span className={result.lirun >= 0 ? common.badgeHangqing : common.badgeQuejia}>
                  {result.lirun >= 0 ? "可做" : "亏损"}
                </span>
              )
            }
            footer={
              result && (
                <span className={common.hint}>
                  期望收入 = {cishu} 次成功 × {formatTong(daoInfo.danjia ?? 0)}（扣 {formatBp(shouxufei.shuilv_bp)} 手续费）
                </span>
              )
            }
          />
          {error && <div className={common.noticeDanger}>{error}</div>}
          {quejiaNames.length > 0 && (
            <div className={common.noticeDanger} role="alert">
              缺价：{quejiaNames.join("、")}（按 0 参与计算，结果仅供参考；可手填或去行情页刷新）
            </div>
          )}
          {result && result.lirun < 0 && (
            <div className={common.notice} role="status">
              当前价格下精炼亏损：可先刷新行情或改一个更接近成本的高阶石价，再判断是否值得做。
            </div>
          )}
          <p className={common.hint}>
            成功率/规则来自 data/wuxingshi.toml（社区口径，待实机校准）；可在行情页查看两种五行石的最新挂单价。
          </p>
        </div>
      </div>
    </div>
  );
}
