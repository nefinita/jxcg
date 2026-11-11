import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as api from "./api/commands";
import ActivityBar, { type Page } from "./components/ActivityBar";
import CalcPage from "./components/CalcPage";
import MarketPage from "./components/MarketPage";
import RankPage from "./components/RankPage";
import SettingsPage from "./components/SettingsPage";
import WuxingshiPage from "./components/WuxingshiPage";
import StatusBar from "./components/StatusBar";
import Toast from "./components/Toast";
import ThemeToggle from "./components/ThemeToggle";
import { IconBug, IconCalc } from "./components/icons";
import { usePersistentState } from "./hooks/usePersistentState";
import { useTheme } from "./hooks/useTheme";
import { useToast } from "./hooks/useToast";
import {
  favoritesSchema,
  isJiageBiao,
  isKuaizhao,
  isSettings,
  isShouxufei,
  pricesSchema,
  snapshotsSchema,
  zhijiOverrideSchema,
} from "./state/schemas";
import { appendSnapshot, getJiage, mergeJiageJilu, setJiage, snapshotOf } from "./utils/jiage";
import {
  CUNDANG_FILE_VERSION,
  DEFAULT_SETTINGS,
  DEFAULT_SHOUXUFEI,
  GITHUB_ISSUES_URL,
  STORAGE_KEYS,
} from "./utils/constants";
import { nowSeconds } from "./utils/format";
import { collectRefreshWupinIds } from "./utils/refresh";
import { dateStamp, downloadJson, pickTextFile } from "./utils/file";
import { formatTong } from "./utils/money";
import type {
  Catalog,
  CundangFile,
  JiageBiao,
  Kuaizhao,
  RefreshProgress,
  Settings,
  Shouxufei,
  Tong,
} from "./types";
import styles from "./App.module.css";

export default function App() {
  const { theme, toggleTheme, setTheme } = useTheme();
  const { toasts, addToast, removeToast } = useToast();

  const [page, setPage] = useState<Page>("calc");
  const [visitedPages, setVisitedPages] = useState<Set<Page>>(() => new Set(["calc"]));
  const [status, setStatus] = useState("就绪");
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [catalogError, setCatalogError] = useState("");
  const [catalogAttempt, setCatalogAttempt] = useState(0);
  const [settings, setSettingsState] = useState<Settings>(DEFAULT_SETTINGS);
  const [settingsReady, setSettingsReady] = useState(false);
  const [focusRecipe, setFocusRecipe] = useState<{ peifangId: string; nonce: number } | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshProgress, setRefreshProgress] = useState<RefreshProgress | null>(null);
  const focusNonce = useRef(0);
  const refreshingRef = useRef(false);
  const autoRefreshTried = useRef(false);

  // 本地持久化：行情缓存 / 收藏 / 快照 / 值集自定义（设置走 Tauri 存档，见下）
  const [prices, setPrices] = usePersistentState<JiageBiao>(STORAGE_KEYS.prices, () => ({}), pricesSchema);
  const [favorites, setFavorites] = usePersistentState<string[]>(STORAGE_KEYS.favorites, () => [], favoritesSchema);
  const [snapshots, setSnapshots] = usePersistentState<Kuaizhao[]>(STORAGE_KEYS.snapshots, () => [], snapshotsSchema);
  const [zhijiOverride, setZhijiOverride] = usePersistentState<Shouxufei | null>(
    STORAGE_KEYS.zhijiOverride,
    () => null,
    zhijiOverrideSchema,
  );

  // 事件回调里读取最新值，避免闭包过期。
  const pricesRef = useRef(prices);
  pricesRef.current = prices;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const catalogRef = useRef(catalog);
  catalogRef.current = catalog;

  // ---- 派生：当前服务器 / 值集 ----
  const fuwuqi = useMemo(
    () => catalog?.fuwuqi.find((entry) => entry.id === settings.fuwuqi_id) ?? catalog?.fuwuqi[0],
    [catalog, settings.fuwuqi_id],
  );
  const fuwuqiId = fuwuqi?.id ?? settings.fuwuqi_id;
  const fuwuqiIdRef = useRef(fuwuqiId);
  fuwuqiIdRef.current = fuwuqiId;
  const zhiji = useMemo(
    () => catalog?.zhiji.find((entry) => entry.id === settings.zhiji_id) ?? catalog?.zhiji[0],
    [catalog, settings.zhiji_id],
  );
  const shouxufei: Shouxufei = zhijiOverride ?? zhiji?.shouxufei ?? DEFAULT_SHOUXUFEI;
  const banben = catalog
    ? `数据 ${catalog.banben.level}级·S${catalog.banben.season} · ${catalog.banben.modified}`
    : undefined;
  // 刷新范围：配方材料优先（去重、最多 200 个），见 utils/refresh.ts。
  const refreshCount = useMemo(() => (catalog ? collectRefreshWupinIds(catalog).length : 0), [catalog]);

  const updateSettings = useCallback((patch: Partial<Settings>) => {
    setSettingsState((previous) => ({ ...previous, ...patch }));
  }, []);

  const navigate = useCallback((next: Page) => {
    setPage(next);
    setVisitedPages((previous) => (previous.has(next) ? previous : new Set([...previous, next])));
  }, []);

  const toggleFavorite = useCallback(
    (key: string) => {
      setFavorites((previous) =>
        previous.includes(key) ? previous.filter((entry) => entry !== key) : [...previous, key],
      );
    },
    [setFavorites],
  );

  // ---- 启动：加载目录 + 设置 ----
  useEffect(() => {
    let cancelled = false;
    setCatalogError("");
    api
      .loadCatalog()
      .then((value) => {
        if (cancelled) return;
        setCatalog(value);
        setStatus(`数据已加载：${value.peifang.length} 个配方 · ${value.wupin.length} 个物品`);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setCatalogError(err instanceof Error ? err.message : String(err));
        setStatus("数据加载失败");
      });
    return () => {
      cancelled = true;
    };
  }, [catalogAttempt]);

  useEffect(() => {
    let cancelled = false;
    api
      .loadSettings()
      .then((value) => {
        if (cancelled) return;
        setSettingsState(value);
        setSettingsReady(true);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setSettingsReady(true);
        addToast(`读取设置失败，使用默认设置：${err instanceof Error ? err.message : String(err)}`, "warning");
      });
    return () => {
      cancelled = true;
    };
  }, [addToast]);

  // 存档里的服务器 id 可能来自旧版本数据：目录加载后回退到 catalog[0]。
  useEffect(() => {
    if (!catalog || !settingsReady) return;
    if (!catalog.fuwuqi.some((entry) => entry.id === settings.fuwuqi_id) && catalog.fuwuqi[0]) {
      setSettingsState((previous) => ({ ...previous, fuwuqi_id: catalog.fuwuqi[0].id }));
      setStatus(`服务器 ${settings.fuwuqi_id} 不在当前数据中，已切换为 ${catalog.fuwuqi[0].mingcheng}`);
    }
  }, [catalog, settingsReady, settings.fuwuqi_id]);

  // ---- 设置自动保存（首次加载后跳过，避免启动即写盘） ----
  const skipSettingsSave = useRef(true);
  useEffect(() => {
    if (!settingsReady) return;
    if (skipSettingsSave.current) {
      skipSettingsSave.current = false;
      return;
    }
    const timer = window.setTimeout(() => {
      api
        .saveSettings(settings)
        .then(() => setStatus("设置已自动保存"))
        .catch((err: unknown) =>
          addToast(`设置保存失败：${err instanceof Error ? err.message : String(err)}`, "warning"),
        );
    }, 600);
    return () => window.clearTimeout(timer);
  }, [settings, settingsReady, addToast]);

  // ---- 切换服务器/启动后拉取本地行情缓存 ----
  useEffect(() => {
    if (!settingsReady || !fuwuqiId) return;
    let cancelled = false;
    api
      .loadPrices(fuwuqiId)
      .then((list) => {
        if (cancelled || list.length === 0) return;
        setPrices((previous) => mergeJiageJilu(previous, fuwuqiId, list));
      })
      .catch((err: unknown) => {
        if (!cancelled) setStatus(`读取本地行情失败：${err instanceof Error ? err.message : String(err)}`);
      });
    return () => {
      cancelled = true;
    };
  }, [fuwuqiId, settingsReady, setPrices]);

  // ---- 行情刷新（含自动快照；失败保留 last-known-good） ----
  const refreshHangqing = useCallback(
    async (silent = false) => {
      const currentCatalog = catalogRef.current;
      if (!currentCatalog || refreshingRef.current) return;
      const targetFuwuqi = currentCatalog.fuwuqi.some((entry) => entry.id === fuwuqiIdRef.current)
        ? fuwuqiIdRef.current
        : currentCatalog.fuwuqi[0]?.id;
      if (!targetFuwuqi) {
        addToast("目录里没有服务器，无法刷新行情", "warning");
        return;
      }
      const ids = collectRefreshWupinIds(currentCatalog);
      if (ids.length === 0) {
        addToast("目录里没有物品，无法刷新行情", "warning");
        return;
      }
      refreshingRef.current = true;
      setRefreshing(true);
      setRefreshProgress({ done: 0, total: ids.length, failed: 0 });
      let unlisten = () => {};
      try {
        unlisten = await api.listenRefreshProgress((progress) => setRefreshProgress(progress));
        const result = await api.refreshPrices(targetFuwuqi, ids);
        const next = mergeJiageJilu(pricesRef.current, targetFuwuqi, result.jiage);
        setPrices(next);
        const snapshot = snapshotOf(next, targetFuwuqi, ids);
        if (Object.keys(snapshot.jiage).length > 0) setSnapshots((previous) => appendSnapshot(previous, snapshot));
        setStatus(`行情刷新完成：成功 ${result.chenggong} / 失败 ${result.shibai}`);
        if (!silent) {
          addToast(
            `行情刷新完成（成功 ${result.chenggong}，失败 ${result.shibai}）`,
            result.shibai > 0 ? "warning" : "success",
          );
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setStatus("行情刷新失败（已保留最近价格）");
        if (silent) setStatus(`自动拉价失败：${message}（可在行情页重试）`);
        else addToast(`行情刷新失败：${message}`, "error");
      } finally {
        unlisten();
        setRefreshing(false);
        setRefreshProgress(null);
        refreshingRef.current = false;
      }
    },
    [addToast, setPrices, setSnapshots],
  );

  // 桌面端首次启动且无缓存时，按设置自动拉一次行情（浏览器 mock 不自动刷）。
  useEffect(() => {
    if (!catalog || !settingsReady || autoRefreshTried.current) return;
    autoRefreshTried.current = true;
    if (!settings.auto_refresh || !api.isTauriRuntime()) return;
    const hasPrice = catalog.wupin.some((wupin) => getJiage(pricesRef.current, fuwuqiIdRef.current, wupin.id));
    if (!hasPrice) void refreshHangqing(true);
  }, [catalog, settingsReady, settings.auto_refresh, refreshHangqing]);

  // ---- 行情写入 / 导入 ----
  const overridePrice = useCallback(
    (wupinId: string, danjia: Tong) => {
      const targetFuwuqi = fuwuqiIdRef.current;
      setPrices((previous) =>
        setJiage(previous, targetFuwuqi, wupinId, {
          danjia,
          gengxin_shijian: nowSeconds(),
          laiyuan: "shoudong",
          shoudong: true,
        }),
      );
      setStatus(`已手改单价：${formatTong(danjia)}`);
      api.upsertPrice(targetFuwuqi, wupinId, danjia).catch((err: unknown) => {
        addToast(`行情写入后端失败（本地已生效）：${err instanceof Error ? err.message : String(err)}`, "warning");
      });
    },
    [addToast, setPrices],
  );

  const importPrices = useCallback(
    (entries: Array<{ wupinId: string; danjia: Tong }>, source: string) => {
      if (entries.length === 0) {
        addToast("没有可导入的价格", "warning");
        return;
      }
      const targetFuwuqi = fuwuqiIdRef.current;
      const shijian = nowSeconds();
      setPrices((previous) => {
        let next = previous;
        for (const entry of entries) {
          next = setJiage(next, targetFuwuqi, entry.wupinId, {
            danjia: entry.danjia,
            gengxin_shijian: shijian,
            laiyuan: "shoudong",
            shoudong: true,
          });
        }
        return next;
      });
      setStatus(`已导入 ${entries.length} 条行情（${source}）`);
      addToast(`已导入 ${entries.length} 条价格（${source}）`, "success");
    },
    [addToast, setPrices],
  );

  // ---- 快照 ----
  const saveSnapshot = useCallback(() => {
    const currentCatalog = catalogRef.current;
    if (!currentCatalog) return;
    const targetFuwuqi = fuwuqiIdRef.current;
    const snapshot = snapshotOf(pricesRef.current, targetFuwuqi, currentCatalog.wupin.map((wupin) => wupin.id));
    const count = Object.keys(snapshot.jiage).length;
    if (count === 0) {
      addToast("当前没有可保存的行情价格", "warning");
      return;
    }
    setSnapshots((previous) => appendSnapshot(previous, snapshot));
    setStatus(`已保存行情快照（${count} 个物品）`);
    addToast(`已保存行情快照（${count} 个物品）`, "success");
  }, [addToast, setSnapshots]);

  const deleteSnapshot = useCallback(
    (shijian: number) => {
      setSnapshots((previous) => previous.filter((snapshot) => snapshot.shijian !== shijian));
      setStatus("已删除行情快照");
    },
    [setSnapshots],
  );

  // ---- 设置页动作 ----
  const saveSettingsNow = useCallback(async () => {
    try {
      await api.saveSettings(settingsRef.current);
      setStatus("设置已保存");
      addToast("设置已保存", "success");
    } catch (err) {
      addToast(`设置保存失败：${err instanceof Error ? err.message : String(err)}`, "error");
    }
  }, [addToast]);

  const reloadCatalog = useCallback(async () => {
    try {
      const next = await api.reloadCatalog();
      setCatalog(next);
      setStatus(`数据已重新加载：${next.peifang.length} 个配方 · ${next.wupin.length} 个物品`);
      addToast("数据目录已重新加载", "success");
    } catch (err) {
      addToast(`重新加载数据失败：${err instanceof Error ? err.message : String(err)}`, "error");
    }
  }, [addToast]);

  const exportArchive = useCallback(() => {
    const archive: CundangFile = {
      version: CUNDANG_FILE_VERSION,
      daochu_shijian: nowSeconds(),
      settings: settingsRef.current,
      zhiji_override: zhijiOverride,
      jiage: pricesRef.current,
      favorites,
      snapshots,
    };
    downloadJson(`jxcg-cundang-${dateStamp()}.json`, archive);
    addToast("存档已导出", "success");
    setStatus("存档已导出");
  }, [zhijiOverride, favorites, snapshots, addToast]);

  const importArchive = useCallback(async () => {
    const picked = await pickTextFile();
    if (!picked) return;
    try {
      const raw: unknown = JSON.parse(picked.text);
      if (typeof raw !== "object" || raw === null) throw new Error("存档不是 JSON 对象");
      const data = raw as Partial<CundangFile> & Record<string, unknown>;
      if (Number(data.version ?? 0) > CUNDANG_FILE_VERSION) {
        throw new Error(`存档版本 ${data.version} 高于当前应用支持的 ${CUNDANG_FILE_VERSION}`);
      }
      if (!isSettings(data.settings)) throw new Error("存档中的设置字段不完整");
      if (!(data.zhiji_override === null || isShouxufei(data.zhiji_override))) {
        throw new Error("存档中的值集覆盖非法");
      }
      if (!isJiageBiao(data.jiage)) throw new Error("存档中的行情数据非法");
      if (!Array.isArray(data.favorites) || !data.favorites.every((value) => typeof value === "string")) {
        throw new Error("存档中的收藏数据非法");
      }
      if (!Array.isArray(data.snapshots) || !data.snapshots.every(isKuaizhao)) {
        throw new Error("存档中的快照数据非法");
      }
      setSettingsState(data.settings);
      setZhijiOverride(data.zhiji_override ?? null);
      setPrices(data.jiage);
      setFavorites(data.favorites);
      setSnapshots(data.snapshots);
      setStatus("存档已导入（设置 / 行情 / 收藏 / 快照）");
      addToast("存档已导入", "success");
    } catch (err) {
      addToast(`导入存档失败：${err instanceof Error ? err.message : String(err)}`, "error");
    }
  }, [setZhijiOverride, setPrices, setFavorites, setSnapshots, addToast]);

  const resetSettings = useCallback(() => {
    setSettingsState(DEFAULT_SETTINGS);
    setZhijiOverride(null);
    setStatus("已恢复默认设置");
    addToast("已恢复默认设置", "success");
  }, [setZhijiOverride, addToast]);

  // 排行页点行 → 计算页选中配方
  const openRecipeFromRank = useCallback(
    (peifangId: string) => {
      focusNonce.current += 1;
      setFocusRecipe({ peifangId, nonce: focusNonce.current });
      navigate("calc");
    },
    [navigate],
  );

  return (
    <div className={styles.container}>
      <div className={styles.body}>
        <ActivityBar currentPage={page} onNavigate={navigate} />
        <div className={styles.main}>
          <header className={styles.header}>
            <div className={styles.logo}>
              <span className={styles.logoMark}>
                <IconCalc size={17} />
              </span>
              <span>JXCG · 剑网3 交易行计算器</span>
            </div>
            <div className={styles.headerActions}>
              {fuwuqi && <span className={styles.serverPill}>{fuwuqi.mingcheng}</span>}
              {api.isMockMode() && <span className={styles.mockPill}>浏览器预览</span>}
              <button
                className={styles.headerBtn}
                onClick={() => window.open(GITHUB_ISSUES_URL, "_blank", "noopener,noreferrer")}
                title="反馈 / Issues"
                aria-label="反馈 / Issues"
              >
                <IconBug size={16} />
              </button>
              <ThemeToggle theme={theme} onToggle={toggleTheme} />
            </div>
          </header>

          {catalogError && (
            <div className={styles.banner} role="alert">
              <span>数据加载失败：{catalogError}</span>
              <button className={styles.bannerBtn} onClick={() => setCatalogAttempt((value) => value + 1)}>
                重试
              </button>
            </div>
          )}
          {!catalog && !catalogError && <div className={styles.loading}>正在加载随包数据…</div>}

          {catalog && (
            <>
              <div className={styles.residentPage} hidden={page !== "calc"}>
                <CalcPage
                  catalog={catalog}
                  settings={settings}
                  updateSettings={updateSettings}
                  shouxufei={shouxufei}
                  prices={prices}
                  fuwuqiId={fuwuqiId}
                  favorites={favorites}
                  onToggleFavorite={toggleFavorite}
                  focusRecipe={focusRecipe}
                  addToast={addToast}
                  setStatus={setStatus}
                />
              </div>
              {visitedPages.has("market") && (
                <div className={styles.residentPage} hidden={page !== "market"}>
                  <MarketPage
                    catalog={catalog}
                    fuwuqiId={fuwuqiId}
                    fuwuqiName={fuwuqi?.mingcheng ?? fuwuqiId}
                    shouxufei={shouxufei}
                    prices={prices}
                    favorites={favorites}
                    onToggleFavorite={toggleFavorite}
                    snapshots={snapshots}
                    onSaveSnapshot={saveSnapshot}
                    onDeleteSnapshot={deleteSnapshot}
                    onOverridePrice={overridePrice}
                    onImportPrices={importPrices}
                    refreshCount={refreshCount}
                    onRefresh={() => void refreshHangqing()}
                    refreshing={refreshing}
                    progress={refreshProgress}
                    addToast={addToast}
                    setStatus={setStatus}
                  />
                </div>
              )}
              {visitedPages.has("wuxingshi") && (
                <div className={styles.residentPage} hidden={page !== "wuxingshi"}>
                  <WuxingshiPage
                    catalog={catalog}
                    settings={settings}
                    updateSettings={updateSettings}
                    shouxufei={shouxufei}
                    prices={prices}
                    fuwuqiId={fuwuqiId}
                    fuwuqiName={fuwuqi?.mingcheng ?? fuwuqiId}
                    onOpenMarket={() => navigate("market")}
                    setStatus={setStatus}
                  />
                </div>
              )}
              {visitedPages.has("rank") && (
                <div className={styles.residentPage} hidden={page !== "rank"}>
                  <RankPage
                    catalog={catalog}
                    settings={settings}
                    shouxufei={shouxufei}
                    prices={prices}
                    fuwuqiId={fuwuqiId}
                    fuwuqiName={fuwuqi?.mingcheng ?? fuwuqiId}
                    onOpenRecipe={openRecipeFromRank}
                    addToast={addToast}
                    setStatus={setStatus}
                  />
                </div>
              )}
              {visitedPages.has("settings") && (
                <div className={styles.residentPage} hidden={page !== "settings"}>
                  <SettingsPage
                    catalog={catalog}
                    settings={settings}
                    updateSettings={updateSettings}
                    shouxufei={shouxufei}
                    zhijiOverride={zhijiOverride}
                    setZhijiOverride={setZhijiOverride}
                    theme={theme}
                    setTheme={setTheme}
                    onSaveSettings={saveSettingsNow}
                    onReloadCatalog={reloadCatalog}
                    onExportArchive={exportArchive}
                    onImportArchive={importArchive}
                    onResetSettings={resetSettings}
                    addToast={addToast}
                    setStatus={setStatus}
                  />
                </div>
              )}
            </>
          )}
        </div>
      </div>
      <StatusBar
        message={status}
        fuwuqi={fuwuqi?.mingcheng}
        banben={banben}
        mock={api.isMockMode()}
      />
      <Toast toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
