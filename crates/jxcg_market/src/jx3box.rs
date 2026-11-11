//! JX3BOX 交易行行情客户端。
//!
//! 实测（2026-11-12）：`GET https://next2.jx3box.com/api/item-price/{物品id}/detail?server={服务器中文名}`
//! 返回 `{ code: 0, data: { prices: [{ created, n_count, unit_price, server }] } }`；
//! HTTPS、免 token、数据新鲜；单物品一次请求，无批量接口；`logs` 历史接口实测为空。
//!
//! 注意：`server` 参数是**服务器中文名**（如 `剑胆琴心`），不是本地 pinyin id；
//! 传错时接口仍返回 `code: 0` 但 `data.prices = null`，会被误判成「没有挂单」，
//! 所以调用方要先从 `data/fuwuqi.toml` 取 `mingcheng` 再传进来。

use std::sync::Arc;
use std::time::Duration;

use jxcg_api::{Guadan, RefreshProgress};
use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::error::{MarketError, Result};

const DEFAULT_BASE: &str = "https://next2.jx3box.com";
const DEFAULT_TIMEOUT_SECS: u64 = 15;
const DEFAULT_RETRY: u32 = 1;

/// Provider 配置。
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// 接口根地址（测试里可指向本地桩）
    pub base: String,
    /// 单请求超时（秒）
    pub timeout_secs: u64,
    /// 并发上限（对上游友好，默认 4，建议不超过 6）
    pub concurrency: usize,
    /// 失败重试次数（默认 1）
    pub retry: u32,
    /// User-Agent（标识来源，不伪装浏览器）
    pub user_agent: String,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            base: DEFAULT_BASE.to_string(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            concurrency: 4,
            retry: DEFAULT_RETRY,
            user_agent: "jxcg/0.1".to_string(),
        }
    }
}

/// JX3BOX 行情客户端（可 Clone，内部 reqwest::Client 共享连接池）。
#[derive(Debug, Clone)]
pub struct Jx3BoxProvider {
    client: reqwest::Client,
    cfg: ProviderConfig,
}

#[derive(Debug, Deserialize)]
struct PriceResponse {
    code: i64,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    data: Option<PriceData>,
}

#[derive(Debug, Deserialize)]
struct PriceData {
    #[serde(default)]
    prices: Option<Vec<PriceRow>>,
}

#[derive(Debug, Deserialize)]
struct PriceRow {
    #[serde(default)]
    created: i64,
    #[serde(default)]
    n_count: u32,
    #[serde(default)]
    unit_price: i64,
}

/// 最小 percent-encoding（查询参数用；只保留 RFC 3986 unreserved 字符）。
fn pct_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        let b = *byte;
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

/// 解析行情响应体（抽成纯函数，方便用录制 JSON 做离线测试）。
fn parse_price_json(text: &str) -> Result<Vec<Guadan>> {
    let body: PriceResponse = serde_json::from_str(text)
        .map_err(|err| MarketError::Parse(format!("响应 JSON 解析失败：{err}")))?;
    if body.code != 0 {
        return Err(MarketError::Upstream(format!(
            "code={} msg={}",
            body.code, body.msg
        )));
    }
    let mut rows: Vec<Guadan> = body
        .data
        .and_then(|data| data.prices)
        .unwrap_or_default()
        .into_iter()
        .map(|row| Guadan {
            shijian: row.created,
            shuliang: row.n_count,
            danjia: row.unit_price,
        })
        .filter(|guadan| guadan.danjia > 0)
        .collect();
    rows.sort_by_key(|guadan| guadan.danjia);
    Ok(rows)
}

/// 详情请求 URL。
fn detail_url(base: &str, wupin_id: &str, fuwuqi_mingcheng: &str) -> String {
    format!(
        "{}/api/item-price/{}/detail?server={}",
        base.trim_end_matches('/'),
        pct_encode(wupin_id),
        pct_encode(fuwuqi_mingcheng)
    )
}

impl Jx3BoxProvider {
    pub fn new(cfg: ProviderConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs.max(1)))
            .user_agent(cfg.user_agent.clone())
            .build()
            .map_err(|err| MarketError::Http(err.to_string()))?;
        Ok(Self { client, cfg })
    }

    /// 配置（调用方常要读并发/超时）。
    pub fn config(&self) -> &ProviderConfig {
        &self.cfg
    }

    /// 拉取单个物品在某服的挂单（按单价升序，已丢弃单价 <= 0 的脏数据）。
    ///
    /// `fuwuqi_mingcheng` 必须是服务器中文名（如 `剑胆琴心`）。
    /// 失败按 `cfg.retry` 重试；仍失败返回错误，由上层决定是否回落 last-known-good。
    pub async fn fetch_detail(
        &self,
        wupin_id: &str,
        fuwuqi_mingcheng: &str,
    ) -> Result<Vec<Guadan>> {
        let url = detail_url(&self.cfg.base, wupin_id, fuwuqi_mingcheng);
        let mut last_error: Option<MarketError> = None;
        for attempt in 0..=self.cfg.retry {
            match self.try_fetch_detail(&url).await {
                Ok(rows) => return Ok(rows),
                Err(err) => {
                    last_error = Some(err);
                    if attempt < self.cfg.retry {
                        // 对上游礼貌：重试前短暂退避
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| MarketError::Http("请求失败".to_string())))
    }

    async fn try_fetch_detail(&self, url: &str) -> Result<Vec<Guadan>> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|err| MarketError::Http(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(MarketError::Http(format!("HTTP {}", resp.status())));
        }
        let text = resp
            .text()
            .await
            .map_err(|err| MarketError::Http(err.to_string()))?;
        parse_price_json(&text)
    }

    /// 批量拉取（带并发上限）；返回与输入同序的结果。
    pub async fn fetch_many(
        &self,
        fuwuqi_mingcheng: &str,
        wupin_ids: &[String],
    ) -> Vec<(String, Result<Vec<Guadan>>)> {
        self.fetch_many_progress(fuwuqi_mingcheng, wupin_ids, |_| {})
            .await
    }

    /// 批量拉取 + 进度回调（每完成一条回调一次，供 Tauri 发进度事件）。
    ///
    /// 回调参数是累计进度（done/total/failed），刷新流程直接转发给前端。
    pub async fn fetch_many_progress<F>(
        &self,
        fuwuqi_mingcheng: &str,
        wupin_ids: &[String],
        mut progress: F,
    ) -> Vec<(String, Result<Vec<Guadan>>)>
    where
        F: FnMut(RefreshProgress),
    {
        let total = wupin_ids.len() as u32;
        let semaphore = Arc::new(Semaphore::new(self.cfg.concurrency.max(1)));
        let mut set: JoinSet<(usize, String, Result<Vec<Guadan>>)> = JoinSet::new();
        for (index, wupin_id) in wupin_ids.iter().enumerate() {
            let provider = self.clone();
            let wupin_id = wupin_id.clone();
            let fuwuqi = fuwuqi_mingcheng.to_string();
            let semaphore = semaphore.clone();
            set.spawn(async move {
                let permit = match semaphore.acquire_owned().await {
                    Ok(permit) => permit,
                    Err(err) => {
                        return (
                            index,
                            wupin_id,
                            Err(MarketError::Http(format!("并发控制失败：{err}"))),
                        );
                    }
                };
                let result = provider.fetch_detail(&wupin_id, &fuwuqi).await;
                drop(permit);
                (index, wupin_id, result)
            });
        }

        let mut done = 0u32;
        let mut failed = 0u32;
        let mut results: Vec<(usize, String, Result<Vec<Guadan>>)> =
            Vec::with_capacity(total as usize);
        while let Some(joined) = set.join_next().await {
            done += 1;
            let entry = match joined {
                Ok(entry) => entry,
                Err(err) => (
                    usize::MAX,
                    String::new(),
                    Err(MarketError::Http(format!("请求任务异常：{err}"))),
                ),
            };
            if entry.2.is_err() {
                failed += 1;
            }
            results.push(entry);
            progress(RefreshProgress {
                done,
                total,
                failed,
            });
        }

        results.sort_by_key(|(index, _, _)| *index);
        results
            .into_iter()
            .filter(|(index, _, _)| *index != usize::MAX)
            .map(|(_, wupin_id, result)| (wupin_id, result))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_DETAIL: &str = include_str!("../tests/fixtures/price_5_30853_jiandanqinxin.json");
    const FIXTURE_EMPTY: &str = include_str!("../tests/fixtures/price_empty.json");

    #[test]
    fn default_config_is_polite() {
        let cfg = ProviderConfig::default();
        assert_eq!(cfg.concurrency, 4);
        assert!(cfg.concurrency <= 6);
        assert_eq!(cfg.timeout_secs, 15);
        assert!(cfg.retry <= 1);
        assert_eq!(cfg.user_agent, "jxcg/0.1");
        assert!(cfg.base.starts_with("https://"));
    }

    #[test]
    fn parse_recorded_detail_fixture() {
        // 录制的真实响应（2026-11-12，5_30853 @ 剑胆琴心）
        let rows = parse_price_json(FIXTURE_DETAIL).expect("录制响应应能解析");
        assert!(!rows.is_empty(), "录制响应里应有挂单");
        assert!(rows.len() <= 15);
        // 按单价升序
        assert!(rows.windows(2).all(|pair| pair[0].danjia <= pair[1].danjia));
        // 单价为正、数量为正的挂单
        assert!(rows.iter().all(|guadan| guadan.danjia > 0));
        assert!(rows.iter().any(|guadan| guadan.shuliang > 0));
        assert!(rows.iter().all(|guadan| guadan.shijian > 0));
    }

    #[test]
    fn parse_empty_prices_fixture() {
        // 「服务器名传错/无挂单」的真实响应：code=0 且 prices=null
        let rows = parse_price_json(FIXTURE_EMPTY).expect("空响应也是合法响应");
        assert!(rows.is_empty());
    }

    #[test]
    fn parse_upstream_error() {
        let text = r#"{"code":1,"msg":"参数错误","data":null}"#;
        let err = parse_price_json(text).expect_err("上游业务错误应报错");
        assert!(matches!(err, MarketError::Upstream(_)));
    }

    #[test]
    fn parse_invalid_json() {
        let err = parse_price_json("<html>502 Bad Gateway</html>").expect_err("非 JSON 应报错");
        assert!(matches!(err, MarketError::Parse(_)));
    }

    #[test]
    fn parse_filters_non_positive_price() {
        let text = r#"{"code":0,"data":{"prices":[
            {"created":1,"n_count":2,"unit_price":0},
            {"created":2,"n_count":3,"unit_price":-5},
            {"created":3,"n_count":4,"unit_price":700}
        ]}}"#;
        let rows = parse_price_json(text).expect("解析失败");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].danjia, 700);
    }

    #[test]
    fn detail_url_encodes_chinese_server() {
        let url = detail_url("https://next2.jx3box.com/", "5_30853", "剑胆琴心");
        assert_eq!(
            url,
            "https://next2.jx3box.com/api/item-price/5_30853/detail?server=%E5%89%91%E8%83%86%E7%90%B4%E5%BF%83"
        );
        // 物品 id 的下划线是 unreserved，不需要编码
        assert!(url.contains("item-price/5_30853"));
    }
}
