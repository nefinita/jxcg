//! JX3BOX 交易行行情客户端。
//!
//! 实测（2026-11-12）：`GET https://next2.jx3box.com/api/item-price/{物品id}/detail?server={服务器}`
//! 返回 `{ code: 0, data: { prices: [{ created, n_count, unit_price, server }] } }`；
//! HTTPS、免 token、数据新鲜；单物品一次请求，无批量接口；`logs` 历史接口实测为空。

use std::sync::Arc;
use std::time::Duration;

use jxcg_api::Guadan;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::error::{MarketError, Result};

const DEFAULT_BASE: &str = "https://next2.jx3box.com";
const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Provider 配置。
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// 接口根地址
    pub base: String,
    /// 单请求超时（秒）
    pub timeout_secs: u64,
    /// 并发上限（对上游友好，默认 4）
    pub concurrency: usize,
    /// User-Agent（便于上游识别；不伪装浏览器）
    pub user_agent: String,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            base: DEFAULT_BASE.to_string(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            concurrency: 4,
            user_agent: "jxcg/0.1 (+https://github.com/nefinita/jxcg)".to_string(),
        }
    }
}

/// JX3BOX 行情客户端。
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

impl Jx3BoxProvider {
    pub fn new(cfg: ProviderConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs))
            .user_agent(cfg.user_agent.clone())
            .build()
            .map_err(|err| MarketError::Http(err.to_string()))?;
        Ok(Self { client, cfg })
    }

    /// 拉取单个物品在某服的挂单（按单价升序）。
    pub async fn fetch_detail(&self, wupin_id: &str, fuwuqi: &str) -> Result<Vec<Guadan>> {
        let url = format!(
            "{}/api/item-price/{}/detail?server={}",
            self.cfg.base,
            pct_encode(wupin_id),
            pct_encode(fuwuqi)
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|err| MarketError::Http(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(MarketError::Http(format!("HTTP {}", resp.status())));
        }
        let body: PriceResponse = resp
            .json()
            .await
            .map_err(|err| MarketError::Parse(err.to_string()))?;
        if body.code != 0 {
            return Err(MarketError::Upstream(format!(
                "code={} msg={}",
                body.code, body.msg
            )));
        }
        let mut rows: Vec<Guadan> = body
            .data
            .and_then(|d| d.prices)
            .unwrap_or_default()
            .into_iter()
            .map(|row| Guadan {
                shijian: row.created,
                shuliang: row.n_count,
                danjia: row.unit_price,
            })
            .collect();
        rows.sort_by_key(|row| row.danjia);
        Ok(rows)
    }

    /// 批量拉取（带并发上限）；返回与输入同序的结果。
    pub async fn fetch_many(
        &self,
        fuwuqi: &str,
        wupin_ids: &[String],
    ) -> Vec<(String, Result<Vec<Guadan>>)> {
        let semaphore = Arc::new(Semaphore::new(self.cfg.concurrency.max(1)));
        let mut set: JoinSet<(usize, String, Result<Vec<Guadan>>)> = JoinSet::new();
        for (index, id) in wupin_ids.iter().enumerate() {
            let provider = self.clone();
            let id = id.clone();
            let fuwuqi = fuwuqi.to_string();
            let semaphore = semaphore.clone();
            set.spawn(async move {
                let _permit = match semaphore.acquire_owned().await {
                    Ok(permit) => permit,
                    Err(err) => {
                        return (
                            index,
                            id,
                            Err(MarketError::Http(format!("并发控制失败：{err}"))),
                        );
                    }
                };
                let result = provider.fetch_detail(&id, &fuwuqi).await;
                (index, id, result)
            });
        }
        let mut results: Vec<(usize, String, Result<Vec<Guadan>>)> = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(value) => results.push(value),
                Err(err) => results.push((usize::MAX, String::new(), Err(MarketError::Http(err.to_string())))),
            }
        }
        results.sort_by_key(|(index, _, _)| *index);
        results
            .into_iter()
            .filter(|(index, _, _)| *index != usize::MAX)
            .map(|(_, id, result)| (id, result))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_friendly() {
        let cfg = ProviderConfig::default();
        assert_eq!(cfg.concurrency, 4);
        assert!(cfg.base.starts_with("https://"));
    }
}
