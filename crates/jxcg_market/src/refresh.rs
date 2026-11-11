//! 行情刷新与 last-known-good 降级。
//!
//! 流程（Tauri `refresh_prices` 直接调 [`refresh_prices`]）：
//! 1. 读本地档案 `hangqing/{服务器}.toml` 作为 last-known-good；
//! 2. 并发拉 JX3BOX（并发/超时/重试由 [`crate::Jx3BoxProvider`] 配置），每完成一条回调进度；
//! 3. 成功 → 覆盖本地档案（含「无挂单」的缺价状态）；失败 → **不覆盖**，保留缓存价并计入 failed；
//! 4. 写回档案，返回 [`RefreshOutcome`]（成功/失败/缺价/降级汇总）。
//!
//! 单物品查询用 [`fetch_xingqing`]：请求失败时自动回落缓存价并标 `jiangji=true`。

use std::collections::BTreeMap;
use std::path::Path;

use jxcg_api::{Guadan, JiageJilu, RefreshProgress, RefreshResult, Tong};

use crate::cache::{self, HangqingDangan};
use crate::error::Result;
use crate::jx3box::Jx3BoxProvider;

/// 一次刷新后的汇总（比 `jxcg_api::RefreshResult` 多出降级/失败详情，供 UI 提示）。
#[derive(Debug, Clone, Default)]
pub struct RefreshOutcome {
    pub fuwuqi_id: String,
    /// 本次刷新时间戳（秒）
    pub gengxin_shijian: i64,
    /// 成功拉到的物品数（含「接口正常但没有挂单」的缺价物品）
    pub chenggong: u32,
    /// 请求失败的物品数（已尝试重试；其中能用缓存价兜底的见 `jiangji`）
    pub shibai: u32,
    /// 缺价物品 id（接口成功但当前无挂单）
    pub quejia: Vec<String>,
    /// 失败但用 last-known-good 缓存价兜底的物品数
    pub jiangji: u32,
    /// 手动覆盖（shoudong）被保留、未被自动价覆盖的物品数
    pub shoudong_baoliu: u32,
    /// 合并后的全部行情（新拉到的 + 用缓存兜底的）；供界面直接展示
    pub jiage: Vec<JiageJilu>,
    /// 失败详情：(物品 id, 中文错误)
    pub shibai_xiangqing: Vec<(String, String)>,
}

impl RefreshOutcome {
    /// 物品总数（成功 + 失败）。
    pub fn total(&self) -> u32 {
        self.chenggong + self.shibai
    }

    /// 是否有可用价格的条目数。
    pub fn youjia_shuliang(&self) -> usize {
        self.jiage
            .iter()
            .filter(|jilu| cache::zuidi_danjia(jilu).is_some())
            .count()
    }

    /// 转成 `jxcg_api::RefreshResult`（Tauri 命令返回值；`jiage` 已含缓存兜底条目）。
    pub fn into_result(self) -> RefreshResult {
        RefreshResult {
            fuwuqi_id: self.fuwuqi_id,
            gengxin_shijian: self.gengxin_shijian,
            chenggong: self.chenggong,
            shibai: self.shibai,
            jiage: self.jiage,
        }
    }
}

/// 刷新某服行情：拉取 + 写缓存 + 返回汇总。
///
/// - `dir`：应用数据目录根（内部使用 `hangqing/{fuwuqi_id}.toml`）；
/// - `fuwuqi_id`：本地服务器 id（pinyin，缓存键），来自 `data/fuwuqi.toml`；
/// - `fuwuqi_mingcheng`：服务器中文名，JX3BOX 接口的 `server` 参数；
/// - `progress`：每完成一条物品回调一次（Tauri 可在这里 emit 进度事件）。
pub async fn refresh_prices<F>(
    provider: &Jx3BoxProvider,
    dir: &Path,
    fuwuqi_id: &str,
    fuwuqi_mingcheng: &str,
    wupin_ids: &[String],
    progress: F,
) -> Result<RefreshOutcome>
where
    F: Fn(RefreshProgress) + Send + Sync,
{
    // 缓存损坏时不让刷新失败：当作空档案继续拉，刷新本身就是修复手段。
    let cached =
        cache::load_dangan(dir, fuwuqi_id).unwrap_or_else(|_| HangqingDangan::new(fuwuqi_id));
    let results = provider
        .fetch_many_progress(fuwuqi_mingcheng, wupin_ids, progress)
        .await;
    let now = crate::now_unix();
    let mut outcome = merge_results(fuwuqi_id, now, cached.wupin, results);

    // 成功/失败合并后的结果写回本地档案（失败项保留旧值 = last-known-good）。
    if !outcome.jiage.is_empty() {
        let mut dangan = HangqingDangan {
            fuwuqi_id: fuwuqi_id.to_string(),
            gengxin_shijian: now,
            wupin: outcome.jiage.clone(),
        };
        dangan.sort();
        cache::save_dangan(dir, &dangan)?;
        outcome.jiage = dangan.wupin;
    }
    Ok(outcome)
}

/// 把抓取结果与旧缓存合并（纯函数，便于离线测试）。
///
/// 语义：
/// - `Ok(挂单)`：覆盖该物品（空挂单也覆盖，表示「当前确实没货」并计入 `quejia`）；
/// - `Err`：保留缓存旧值（last-known-good），有旧价则 `jiangji += 1`，不更新该条时间戳。
pub fn merge_results(
    fuwuqi_id: &str,
    now: i64,
    cached: Vec<JiageJilu>,
    results: Vec<(String, Result<Vec<Guadan>>)>,
) -> RefreshOutcome {
    let mut map: BTreeMap<String, JiageJilu> = cached
        .into_iter()
        .map(|jilu| (jilu.wupin_id.clone(), jilu))
        .collect();
    let mut outcome = RefreshOutcome {
        fuwuqi_id: fuwuqi_id.to_string(),
        gengxin_shijian: now,
        ..RefreshOutcome::default()
    };

    for (wupin_id, result) in results {
        match result {
            Ok(guadan) => {
                outcome.chenggong += 1;
                // 手动覆盖优先：用户手填的价不被自动行情覆盖（PLAN D2）。
                if map
                    .get(&wupin_id)
                    .map(|jilu| jilu.shoudong)
                    .unwrap_or(false)
                {
                    outcome.shoudong_baoliu += 1;
                    continue;
                }
                if guadan.is_empty() {
                    outcome.quejia.push(wupin_id.clone());
                }
                map.insert(
                    wupin_id.clone(),
                    JiageJilu {
                        wupin_id,
                        fuwuqi_id: fuwuqi_id.to_string(),
                        guadan,
                        gengxin_shijian: now,
                        laiyuan: "jx3box".to_string(),
                        shoudong: false,
                    },
                );
            }
            Err(err) => {
                outcome.shibai += 1;
                outcome
                    .shibai_xiangqing
                    .push((wupin_id.clone(), err.to_string()));
                let has_last_known_good = map
                    .get(&wupin_id)
                    .map(|jilu| !jilu.shoudong && cache::zuidi_danjia(jilu).is_some())
                    .unwrap_or(false);
                if has_last_known_good {
                    outcome.jiangji += 1;
                }
            }
        }
    }

    outcome.quejia.sort();
    outcome.quejia.dedup();
    let mut jiage: Vec<JiageJilu> = map.into_values().collect();
    jiage.sort_by_key(|jilu| cache::wupin_sort_key(&jilu.wupin_id));
    outcome.jiage = jiage;
    outcome
}

/// 单物品行情结果（含降级标记）。
#[derive(Debug, Clone)]
pub struct XingqingJieguo {
    pub wupin_id: String,
    pub fuwuqi_id: String,
    pub guadan: Vec<Guadan>,
    /// 最低挂单价（铜/件）；无挂单为 None
    pub zuidi_danjia: Option<Tong>,
    pub gengxin_shijian: i64,
    /// `jx3box` 实时拉到 / `huancun` 用本地缓存兜底
    pub laiyuan: String,
    /// 是否降级（true = 用了 last-known-good）
    pub jiangji: bool,
}

/// 查单个物品行情：优先联网，失败回落本地缓存价（last-known-good）。
pub async fn fetch_xingqing(
    provider: &Jx3BoxProvider,
    dir: &Path,
    fuwuqi_id: &str,
    fuwuqi_mingcheng: &str,
    wupin_id: &str,
) -> Result<XingqingJieguo> {
    match provider.fetch_detail(wupin_id, fuwuqi_mingcheng).await {
        Ok(guadan) => Ok(XingqingJieguo {
            wupin_id: wupin_id.to_string(),
            fuwuqi_id: fuwuqi_id.to_string(),
            zuidi_danjia: guadan
                .iter()
                .map(|item| item.danjia)
                .filter(|danjia| *danjia > 0)
                .min(),
            guadan,
            gengxin_shijian: crate::now_unix(),
            laiyuan: "jx3box".to_string(),
            jiangji: false,
        }),
        Err(online_error) => {
            let dangan = match cache::load_dangan(dir, fuwuqi_id) {
                Ok(dangan) => dangan,
                // 缓存也读不了：把联网错误原样抛给调用方（更有信息量）
                Err(_) => return Err(online_error),
            };
            match dangan.find(wupin_id) {
                Some(jilu) if cache::zuidi_danjia(jilu).is_some() => Ok(XingqingJieguo {
                    wupin_id: wupin_id.to_string(),
                    fuwuqi_id: fuwuqi_id.to_string(),
                    guadan: jilu.guadan.clone(),
                    zuidi_danjia: cache::zuidi_danjia(jilu),
                    gengxin_shijian: jilu.gengxin_shijian,
                    laiyuan: if jilu.laiyuan.is_empty() {
                        "huancun".to_string()
                    } else {
                        jilu.laiyuan.clone()
                    },
                    jiangji: true,
                }),
                _ => Err(online_error),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MarketError;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.cache/market-test")
            .join(format!("{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("创建临时目录失败");
        dir
    }

    fn jilu(wupin_id: &str, danjia: i64, gengxin_shijian: i64) -> JiageJilu {
        JiageJilu {
            wupin_id: wupin_id.to_string(),
            fuwuqi_id: "jiandanqinxin".to_string(),
            guadan: vec![Guadan {
                shijian: gengxin_shijian,
                shuliang: 10,
                danjia,
            }],
            gengxin_shijian,
            laiyuan: "jx3box".to_string(),
            shoudong: false,
        }
    }

    #[test]
    fn merge_success_overwrites_and_flags_quejia() {
        let cached = vec![jilu("5_1", 100, 1)];
        let results = vec![
            (
                "5_1".to_string(),
                Ok(vec![Guadan {
                    shijian: 5,
                    shuliang: 2,
                    danjia: 300,
                }]),
            ),
            ("5_2".to_string(), Ok(vec![])),
        ];
        let outcome = merge_results("jiandanqinxin", 9_999, cached, results);
        assert_eq!(outcome.chenggong, 2);
        assert_eq!(outcome.shibai, 0);
        assert_eq!(outcome.quejia, vec!["5_2".to_string()]);
        assert_eq!(outcome.jiangji, 0);
        assert_eq!(outcome.total(), 2);
        let first = outcome
            .jiage
            .iter()
            .find(|item| item.wupin_id == "5_1")
            .expect("5_1 应在结果里");
        assert_eq!(first.guadan[0].danjia, 300);
        assert_eq!(first.gengxin_shijian, 9_999);
        assert_eq!(outcome.youjia_shuliang(), 1);
    }

    #[test]
    fn merge_failure_keeps_last_known_good() {
        let cached = vec![jilu("5_1", 100, 111)];
        let results = vec![(
            "5_1".to_string(),
            Err(MarketError::Http("超时".to_string())),
        )];
        let outcome = merge_results("jiandanqinxin", 9_999, cached, results);
        assert_eq!(outcome.chenggong, 0);
        assert_eq!(outcome.shibai, 1);
        assert_eq!(outcome.jiangji, 1);
        assert_eq!(outcome.total(), 1);
        assert_eq!(outcome.shibai_xiangqing.len(), 1);
        assert_eq!(outcome.shibai_xiangqing[0].0, "5_1");
        // 缓存价保留，时间戳/来源不变（重要：失败不能清掉 last-known-good）
        let kept = &outcome.jiage[0];
        assert_eq!(kept.guadan[0].danjia, 100);
        assert_eq!(kept.gengxin_shijian, 111);
        assert_eq!(kept.laiyuan, "jx3box");
    }

    #[test]
    fn merge_failure_without_cache_adds_nothing() {
        let results = vec![(
            "5_9".to_string(),
            Err(MarketError::Http("断网".to_string())),
        )];
        let outcome = merge_results("jiandanqinxin", 1, vec![], results);
        assert_eq!(outcome.shibai, 1);
        assert_eq!(outcome.jiangji, 0);
        assert!(outcome.jiage.is_empty());
    }

    #[test]
    fn merge_keeps_manual_override() {
        let mut manual = jilu("5_1", 888, 50);
        manual.shoudong = true;
        manual.laiyuan = "shoudong".to_string();
        let results = vec![(
            "5_1".to_string(),
            Ok(vec![Guadan {
                shijian: 99,
                shuliang: 3,
                danjia: 300,
            }]),
        )];
        let outcome = merge_results("jiandanqinxin", 9_999, vec![manual], results);
        assert_eq!(outcome.chenggong, 1);
        assert_eq!(outcome.shoudong_baoliu, 1);
        assert!(outcome.quejia.is_empty());
        let kept = &outcome.jiage[0];
        assert!(kept.shoudong, "手动覆盖不应被自动价覆盖");
        assert_eq!(kept.guadan[0].danjia, 888);
    }

    #[test]
    fn into_result_maps_dto() {
        let outcome = RefreshOutcome {
            fuwuqi_id: "x".to_string(),
            gengxin_shijian: 7,
            chenggong: 2,
            shibai: 1,
            jiage: vec![jilu("5_1", 1, 7)],
            ..RefreshOutcome::default()
        };
        let result = outcome.into_result();
        assert_eq!(result.fuwuqi_id, "x");
        assert_eq!(result.gengxin_shijian, 7);
        assert_eq!(result.chenggong, 2);
        assert_eq!(result.shibai, 1);
        assert_eq!(result.jiage.len(), 1);
    }

    /// 用连不上的本地地址模拟断网：失败后应回落本地缓存（不依赖外网）。
    #[tokio::test]
    async fn fetch_xingqing_falls_back_to_cache() {
        let dir = temp_dir("fallback");
        cache::save_prices(&dir, "jiandanqinxin", &[jilu("5_30853", 1_234, 777)])
            .expect("写缓存失败");
        let cfg = crate::ProviderConfig {
            base: "http://127.0.0.1:9".to_string(),
            timeout_secs: 1,
            concurrency: 1,
            retry: 0,
            user_agent: "jxcg-test".to_string(),
        };
        let provider = crate::Jx3BoxProvider::new(cfg).expect("provider 构建失败");
        let xingqing = fetch_xingqing(&provider, &dir, "jiandanqinxin", "剑胆琴心", "5_30853")
            .await
            .expect("应回落缓存");
        assert!(xingqing.jiangji);
        assert_eq!(xingqing.zuidi_danjia, Some(1_234));
        assert_eq!(xingqing.gengxin_shijian, 777);
        assert_eq!(xingqing.laiyuan, "jx3box");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 真实联网冒烟（默认 ignored，手动执行）：
    /// `cargo test -p jxcg_market --lib live_refresh -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "需要联网：cargo test -p jxcg_market --lib live_refresh -- --ignored --nocapture"]
    async fn live_refresh_touches_upstream() {
        let dir = temp_dir("live");
        let provider = crate::Jx3BoxProvider::new(crate::ProviderConfig {
            concurrency: 2,
            ..crate::ProviderConfig::default()
        })
        .expect("provider 构建失败");
        let wupin_ids = vec!["5_30853".to_string(), "5_30845".to_string()];
        let outcome = refresh_prices(
            &provider,
            &dir,
            "jiandanqinxin",
            "剑胆琴心",
            &wupin_ids,
            |progress| {
                println!(
                    "  进度 done={}/{} failed={}",
                    progress.done, progress.total, progress.failed
                )
            },
        )
        .await
        .expect("刷新失败");
        println!(
            "  成功 {} / 失败 {} / 缺价 {} / 降级 {}",
            outcome.chenggong,
            outcome.shibai,
            outcome.quejia.len(),
            outcome.jiangji
        );
        assert_eq!(outcome.total(), 2);
        assert!(
            outcome.chenggong >= 1,
            "至少一个物品应拉到行情（网络不佳时重跑）"
        );
        // 刷新后本地档案应能读回
        let loaded = cache::load_prices(&dir, "jiandanqinxin").expect("读缓存失败");
        assert!(!loaded.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 断网且没有缓存 → 返回联网错误（而不是假装成功）。
    #[tokio::test]
    async fn fetch_xingqing_without_cache_returns_error() {
        let dir = temp_dir("nocache");
        let cfg = crate::ProviderConfig {
            base: "http://127.0.0.1:9".to_string(),
            timeout_secs: 1,
            concurrency: 1,
            retry: 0,
            user_agent: "jxcg-test".to_string(),
        };
        let provider = crate::Jx3BoxProvider::new(cfg).expect("provider 构建失败");
        let err = fetch_xingqing(&provider, &dir, "jiandanqinxin", "剑胆琴心", "5_1")
            .await
            .expect_err("无缓存应报错");
        assert!(matches!(err, MarketError::Http(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
