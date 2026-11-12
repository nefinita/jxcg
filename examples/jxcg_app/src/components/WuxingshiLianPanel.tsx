import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api/commands";
import MoneyInput from "./MoneyInput";
import ResultCard, { type ResultItem } from "./ResultCard";
import common from "../styles/common.module.css";
import styles from "./WuxingshiLianPanel.module.css";
import { usePersistentState } from "../hooks/usePersistentState";
import { DEFAULT_WUXINGSHI_LIAN_PREFS, wuxingshiLianPrefsSchema } from "../state/schemas";
import { STORAGE_KEYS } from "../utils/constants";
import { clampInt, formatBp, formatQiwang } from "../utils/format";
import { formatTong } from "../utils/money";
import { quanliangJiageMap } from "../utils/jiage";
import type {
  Catalog,
  JiageBiao,
  Settings,
  Shouxufei,
  Wupin,
  WuxingshiLianResult,
} from "../types";

export interface WuxingshiLianPanelProps {
  catalog: Catalog;
  settings: Settings;
  updateSettings: (patch: Partial<Settings>) => void;
  shouxufei: Shouxufei;
  prices: JiageBiao;
  fuwuqiId: string;
  fuwuqiName: string;
  setStatus: (message: string) => void;
}

const LEVEL_CHARS: Record<string, number> = {
  一: 1,
  二: 2,
  三: 3,
  四: 4,
  五: 5,
  六: 6,
  七: 7,
  八: 8,
};

/** 从「五行石（六级）」名称解析等级；不是五行石返回 null。 */
function stoneLevel(wupin: Wupin): number | null {
  if (wupin.pinlei !== "wuxingshi") return null;
  const matched = /五行石（([一二三四五六七八])级）/.exec(wupin.mingcheng);
  return matched ? (LEVEL_CHARS[matched[1]] ?? null) : null;
}

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

/** 五行石「一键合成到六级」主面板（task-10）。 */
export default function WuxingshiLianPanel({
  catalog,
  settings,
  updateSettings,
  shouxufei,
  prices,
  fuwuqiId,
  fuwuqiName,
  setStatus,
}: WuxingshiLianPanelProps) {
  const [prefs, setPrefs] = usePersistentState(
    STORAGE_KEYS.wuxingshiLianPrefs,
    () => DEFAULT_WUXINGSHI_LIAN_PREFS,
    wuxingshiLianPrefsSchema,
  );
  const [result, setResult] = useState<WuxingshiLianResult | null>(null);
  const [error, setError] = useState("");
  const [calculating, setCalculating] = useState(false);
  const rev = useRef(0);

  const fangans = catalog.wuxingshi_fangan ?? [];
  const hechengs = catalog.wuxingshi_hecheng ?? [];
  const selectedFangan = fangans.find((item) => item.id === prefs.fangan_id) ?? fangans[0];

  useEffect(() => {
    if (selectedFangan && selectedFangan.id !== prefs.fangan_id) {
      setPrefs((previous) => ({ ...previous, fangan_id: selectedFangan.id }));
    }
  }, [selectedFangan, prefs.fangan_id, setPrefs]);

  // 可合成目标：方案内的主产物（按等级从高到低），外加目录里的七/八级（无配方时禁用）。
  const targetOptions = useMemo(() => {
    const enabled = new Set(hechengs.map((hecheng) => hecheng.chanpin));
    const options: Array<{ id: string; label: string; level: number; enabled: boolean }> = [];
    for (const wupin of catalog.wupin) {
      const level = stoneLevel(wupin);
      if (level === null || level < 3) continue;
      const available = enabled.has(wupin.id);
      if (!available && level !== 7 && level !== 8) continue;
      options.push({ id: wupin.id, label: wupin.mingcheng, level, enabled: available });
    }
    return options.sort((a, b) => b.level - a.level);
  }, [catalog.wupin, hechengs]);

  const defaultTarget = useMemo(() => {
    // 默认六级；若数据缺六级则取最高可合成等级。
    return (
      targetOptions.find((option) => option.level === 6 && option.enabled) ??
      targetOptions.find((option) => option.enabled) ??
      targetOptions[0]
    );
  }, [targetOptions]);

  const selectedTargetId = targetOptions.some((option) => option.id === prefs.mubiao_wupin_id)
    ? prefs.mubiao_wupin_id
    : (defaultTarget?.id ?? "5_24428");
  const selectedTarget = targetOptions.find((option) => option.id === selectedTargetId);
  const targetEnabled = selectedTarget?.enabled ?? false;

  useEffect(() => {
    if (defaultTarget && !targetOptions.some((option) => option.id === prefs.mubiao_wupin_id)) {
      setPrefs((previous) => ({ ...previous, mubiao_wupin_id: defaultTarget.id }));
    }
  }, [defaultTarget, targetOptions, prefs.mubiao_wupin_id, setPrefs]);

  const jiage = useMemo(() => quanliangJiageMap(prices, fuwuqiId), [prices, fuwuqiId]);
  const baojiBp = Math.max(0, Math.min(10_000, Math.trunc(prefs.baoji_bp)));

  useEffect(() => {
    if (!selectedFangan || !targetEnabled) {
      setResult(null);
      setCalculating(false);
      return;
    }
    const currentRev = ++rev.current;
    setCalculating(true);
    setError("");
    const timer = window.setTimeout(() => {
      api
        .calculateWuxingshi({
          fangan_id: selectedFangan.id,
          mubiao_wupin_id: selectedTargetId,
          shuliang: Math.max(1, prefs.shuliang),
          jingli_danjia: settings.jingli_danjia,
          shouxufei,
          jiage,
          baoji_bp: baojiBp,
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
    }, 180);
    return () => {
      window.clearTimeout(timer);
      ++rev.current;
    };
  }, [
    selectedFangan,
    targetEnabled,
    selectedTargetId,
    prefs.shuliang,
    settings.jingli_danjia,
    shouxufei,
    jiage,
    baojiBp,
  ]);

  const wupinName = (id: string): string => catalog.wupin.find((wupin) => wupin.id === id)?.mingcheng ?? id;
  const eightLevelId = useMemo(
    () => catalog.wupin.find((wupin) => stoneLevel(wupin) === 8)?.id ?? "5_24430",
    [catalog.wupin],
  );
  const eightLevel = result?.fuchanpin.find((row) => row.wupin_id === eightLevelId);
  const quejiaNames = (result?.quejia ?? []).map(wupinName);
  const shuliang = Math.max(1, prefs.shuliang);
  const rewardPerItem = result ? Math.round(result.lirun / shuliang) : 0;

  const resultItems: ResultItem[] = result
    ? [
        {
          label: "单颗净成本",
          value: result.danjia,
          kind: "money",
          emphasis: true,
          hint: "材料 + 精力 + 加工费 − 副产物折价",
        },
        {
          label: "单颗利润",
          value: rewardPerItem,
          kind: "money",
          tone: rewardPerItem < 0 ? "negative" : "positive",
          hint: "扣 5% 手续费后",
        },
        { label: "市场价", value: result.shichangjia, kind: "money" },
        { label: "利润率", value: result.lirunlv_bp, kind: "bp", tone: result.lirunlv_bp < 0 ? "negative" : "default" },
        { label: "保本价", value: result.baobenjia, kind: "money", hint: "按手续费反推的最小挂单价" },
        { label: "副产物期望收益", value: result.fuchanpin_shouru, kind: "money", hint: "已冲抵成本" },
        { label: "净总成本", value: result.zong_chengben, kind: "money" },
        { label: "目标数量", value: shuliang, kind: "number", hint: `共 ${formatTong(result.lirun)} 利润` },
      ]
    : [];

  if (fangans.length === 0) {
    return (
      <div className={common.notice}>
        随包数据还没有五行石合成方案（data/wuxingshi.toml 的 `[[fangan]]` 为空）。
      </div>
    );
  }

  return (
    <div className={styles.layout}>
      {/* 参数 */}
      <section className={common.card}>
        <div className={common.cardTitle}>
          <span>一键合成参数</span>
          <span className={common.hint}>{formatBp(shouxufei.shuilv_bp)} 手续费</span>
        </div>
        <div className={styles.form}>
          <label className={common.field}>
            <span className={common.fieldLabel}>合成方案</span>
            <select
              className={common.select}
              value={selectedFangan?.id ?? ""}
              onChange={(event) => setPrefs((previous) => ({ ...previous, fangan_id: event.target.value }))}
              aria-label="合成方案"
            >
              {fangans.map((fangan) => (
                <option key={fangan.id} value={fangan.id}>
                  {fangan.mingcheng}
                </option>
              ))}
            </select>
            {selectedFangan?.note && <span className={common.hint}>{selectedFangan.note}</span>}
          </label>

          <label className={common.field}>
            <span className={common.fieldLabel}>目标物品（默认六级；七/八级暂无合成配方时禁用）</span>
            <select
              className={common.select}
              value={selectedTargetId}
              onChange={(event) => setPrefs((previous) => ({ ...previous, mubiao_wupin_id: event.target.value }))}
              aria-label="目标物品"
            >
              {targetOptions.map((option) => (
                <option key={option.id} value={option.id} disabled={!option.enabled}>
                  {option.label}
                  {option.enabled ? "" : "（暂无合成配方）"}
                </option>
              ))}
            </select>
          </label>

          <div className={styles.paramGrid}>
            <label className={common.field}>
              <span className={common.fieldLabel}>目标数量</span>
              <input
                className={common.numberInput}
                type="number"
                min={1}
                max={999}
                value={prefs.shuliang}
                onChange={(event) =>
                  setPrefs((previous) => ({ ...previous, shuliang: clampInt(Number(event.target.value), 1, 999) }))
                }
                aria-label="目标数量"
              />
            </label>
            <label className={common.field}>
              <span className={common.fieldLabel}>精力单价（铜/点）</span>
              <MoneyInput
                ariaLabel="精力单价"
                value={settings.jingli_danjia}
                onChange={(tong) => updateSettings({ jingli_danjia: tong })}
              />
            </label>
          </div>

          <label className={common.field}>
            <span className={common.fieldLabel}>合 6 出 8 级概率（万分比）</span>
            <div className={common.fieldRow}>
              <input
                className={common.numberInput}
                type="number"
                min={0}
                max={10000}
                step={1}
                value={prefs.baoji_bp}
                onChange={(event) =>
                  setPrefs((previous) => ({ ...previous, baoji_bp: clampInt(Number(event.target.value), 0, 10_000) }))
                }
                aria-label="合6出8级概率"
              />
              <span className={common.hint}>
                {formatBp(baojiBp)}（面板概率待校准；填 0 忽略，例：0.05% = 5）
              </span>
            </div>
          </label>

          <p className={common.hint}>
            期望口径：主产物概率 &lt;100% 时投入按 1/p 放大；副产物（2/5/6/8 级）按 {fuwuqiName} 行情折价冲抵成本；
            净成本 = 材料 + 精力折金 + 加工费 − 副产物收益。
          </p>
          {!targetEnabled && (
            <div className={common.noticeDanger} role="alert">
              该目标等级暂无合成配方（数据待补），可先看五/六级。
            </div>
          )}
          {error && <div className={common.noticeDanger}>{error}</div>}
          <p className={common.hint}>{calculating ? "计算中…" : result ? `已按 ${result.buzhou.length} 步合成链计算` : ""}</p>
        </div>
      </section>

      {/* 结果 */}
      <div className={styles.resultStack}>
        <ResultCard
          title={result ? `一键合成 · ${result.mubiao_mingcheng}` : "一键合成"}
          items={resultItems}
          emptyText={calculating ? "计算中…" : "选择方案与目标后自动计算"}
          extra={
            result && (
              <span className={result.lirun >= 0 ? common.badgeHangqing : common.badgeQuejia}>
                {result.lirun >= 0 ? "有利润" : "亏损"}
              </span>
            )
          }
          footer={
            result && (
              <span className={common.hint}>
                市场价 {formatTong(result.shichangjia)} × {shuliang} − 手续费 = 到手{" "}
                {formatTong(result.shichangjia * shuliang - jisuanFee(result.shichangjia * shuliang, shouxufei))}
              </span>
            )
          }
        />
        {quejiaNames.length > 0 && (
          <div className={common.noticeDanger} role="alert">
            缺价：{quejiaNames.join("、")}（按 0 计，收益/成本不完整；可在行情页一键刷新或手动录价）
          </div>
        )}
        {eightLevel ? (
          <div className={common.notice} role="status">
            合 6 出 8 级：期望 {formatQiwang(eightLevel.qiwang)} 颗 × {formatTong(eightLevel.danjia)} = +
            {formatTong(eightLevel.shouru)}（约 {formatTong(Math.round(eightLevel.shouru / shuliang))}/颗，概率待校准）
          </div>
        ) : (
          <div className={common.notice} role="status">
            填 0 忽略 8 级暴击收益；面板概率待校准（例：0.05% = 5 万分比，约 +12.8 金/颗）。
          </div>
        )}
      </div>

      {/* 消耗 + 副产物 + 分步 */}
      <div className={styles.detailStack}>
        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>总消耗</span>
            <span className={common.hint}>期望值</span>
          </div>
          {result ? (
            <>
              <table className={common.table}>
                <thead>
                  <tr>
                    <th>采购原料</th>
                    <th className={common.num}>期望数量</th>
                    <th className={common.num}>单价</th>
                    <th className={common.num}>小计</th>
                    <th>来源</th>
                  </tr>
                </thead>
                <tbody>
                  {result.cailiao.map((row) => (
                    <tr key={row.wupin_id}>
                      <td>{row.mingcheng}</td>
                      <td className={common.num}>{row.shuliang}</td>
                      <td className={common.num}>{formatTong(row.danjia)}</td>
                      <td className={common.num}>{formatTong(row.xiaojia)}</td>
                      <td>
                        <span className={LAIGUAN_BADGE[row.laiyuan] ?? common.badge}>
                          {LAIGUAN_LABEL[row.laiyuan] ?? row.laiyuan}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <dl className={styles.summary}>
                <div>
                  <dt>材料成本</dt>
                  <dd>{formatTong(result.cailiao_chengben)}</dd>
                </div>
                <div>
                  <dt>精力</dt>
                  <dd>
                    {result.jingli} 点（{formatTong(result.jingli_chengben)}）
                  </dd>
                </div>
                <div>
                  <dt>加工费</dt>
                  <dd>{formatTong(result.jiagongfei)}</dd>
                </div>
                <div>
                  <dt>副产物收益</dt>
                  <dd>−{formatTong(result.fuchanpin_shouru)}</dd>
                </div>
                <div>
                  <dt>净总成本</dt>
                  <dd>{formatTong(result.zong_chengben)}</dd>
                </div>
                <div>
                  <dt>单颗净成本</dt>
                  <dd>{formatTong(result.danjia)}</dd>
                </div>
              </dl>
            </>
          ) : (
            <p className={common.empty}>{calculating ? "计算中…" : "等待计算"}</p>
          )}
        </section>

        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>副产物期望收益</span>
            <span className={common.hint}>{result ? `${result.fuchanpin.length} 种` : ""}</span>
          </div>
          {result && result.fuchanpin.length > 0 ? (
            <table className={common.table}>
              <thead>
                <tr>
                  <th>副产物</th>
                  <th className={common.num}>期望数量</th>
                  <th className={common.num}>行情价</th>
                  <th className={common.num}>期望收益</th>
                </tr>
              </thead>
              <tbody>
                {result.fuchanpin.map((row) => (
                  <tr key={row.wupin_id}>
                    <td>{row.mingcheng}</td>
                    <td className={common.num}>{formatQiwang(row.qiwang)}</td>
                    <td className={common.num}>{formatTong(row.danjia)}</td>
                    <td className={common.num}>{formatTong(row.shouru)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className={common.hint}>无副产物（或行情缺价按 0 计）。</p>
          )}
          {result && result.fuchanpin.length > 0 && (
            <p className={common.hint}>
              合计 {formatTong(result.fuchanpin_shouru)}，已从净成本中冲抵；2/5/6 级来自低一级合成副产物，8 级来自合 6 暴击。
            </p>
          )}
        </section>

        <section className={common.card}>
          <div className={common.cardTitle}>
            <span>分步明细</span>
            <span className={common.hint}>按数据文件顺序</span>
          </div>
          {result && result.buzhou.length > 0 ? (
            <table className={common.table}>
              <thead>
                <tr>
                  <th>步骤</th>
                  <th>产出</th>
                  <th className={common.num}>期望次数</th>
                  <th className={common.num}>精力</th>
                  <th className={common.num}>加工费</th>
                </tr>
              </thead>
              <tbody>
                {result.buzhou.map((row) => (
                  <tr key={row.hecheng_id}>
                    <td>{row.hecheng_id}</td>
                    <td>{row.mingcheng}</td>
                    <td className={common.num}>{formatQiwang(row.cishu)}</td>
                    <td className={common.num}>{row.jingli}</td>
                    <td className={common.num}>{formatTong(row.jiagongfei)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className={common.hint}>暂无步骤（目标等级没有合成配方）。</p>
          )}
        </section>
      </div>
    </div>
  );
}

/** 手续费（HALF_UP 到铜），仅用于结果卡 footer 展示。 */
function jisuanFee(amount: number, cfg: Shouxufei): number {
  if (amount <= 0 || cfg.shuilv_bp <= 0) return 0;
  const raw = amount * cfg.shuilv_bp;
  let fee = Math.floor(raw / 10_000);
  if (cfg.quzheng === "ceil") fee = Math.ceil(raw / 10_000);
  else if (cfg.quzheng === "half_up") fee = Math.floor((raw + 5_000) / 10_000);
  return Math.max(0, Math.min(fee, amount));
}
