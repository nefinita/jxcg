//! 对上游友好的 HTTP 客户端：超时 / User-Agent / 失败重试 1 次 / 原始 JSON 缓存。
//!
//! 缓存只服务本工具的重复运行（开发调试、断点续跑），
//! 默认目录 `.cache/jx3box`（已在 .gitignore 中），不会随包/入库。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::error::{Error, Result};

/// 失败重试次数（首次 + 1 次重试）。
const RETRY_TIMES: u32 = 1;

#[derive(Clone)]
pub struct HttpClient {
    inner: Arc<Inner>,
}

struct Inner {
    client: reqwest::Client,
    cache_dir: Option<PathBuf>,
    /// 离线模式：只读缓存，不联网。
    offline: bool,
    /// 忽略已有缓存，强制重新拉取（仍会写缓存）。
    refresh_cache: bool,
    /// 网络请求次数（含失败重试）。
    network_requests: AtomicU64,
    /// 缓存命中次数。
    cache_hits: AtomicU64,
    /// 重试后仍失败的 URL 数。
    failures: AtomicU64,
}

impl HttpClient {
    pub fn new(
        timeout_secs: u64,
        cache_dir: Option<PathBuf>,
        offline: bool,
        refresh_cache: bool,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs.max(1)))
            // 不伪装浏览器：便于上游识别来源（见 PLAN §7.1 接入原则）。
            .user_agent("jxcg/0.1")
            .build()?;
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                cache_dir,
                offline,
                refresh_cache,
                network_requests: AtomicU64::new(0),
                cache_hits: AtomicU64::new(0),
                failures: AtomicU64::new(0),
            }),
        })
    }

    /// 取 JSON；优先读缓存，失败自动重试 1 次。
    pub async fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        if let Some(path) = self.cache_path(url)
            && !self.inner.refresh_cache
            && path.is_file()
        {
            let text = std::fs::read_to_string(&path)?;
            if let Ok(value) = serde_json::from_str(&text) {
                self.inner.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(value);
            }
            // 缓存坏了就当作没有，继续走网络。
        }
        if self.inner.offline {
            return Err(Error::Http(format!("离线模式缺少缓存：{url}")));
        }

        let mut last_error: Option<Error> = None;
        for attempt in 0..=RETRY_TIMES {
            self.inner.network_requests.fetch_add(1, Ordering::Relaxed);
            match self.try_get(url).await {
                Ok(value) => {
                    if let Some(path) = self.cache_path(url) {
                        write_cache(&path, &value);
                    }
                    return Ok(value);
                }
                Err(err) => {
                    if attempt < RETRY_TIMES {
                        // 对上游礼貌：重试前短暂退避。
                        tokio::time::sleep(Duration::from_millis(800)).await;
                    }
                    last_error = Some(err);
                }
            }
        }
        self.inner.failures.fetch_add(1, Ordering::Relaxed);
        Err(last_error.unwrap_or_else(|| Error::Http(format!("请求失败：{url}"))))
    }

    async fn try_get(&self, url: &str) -> Result<serde_json::Value> {
        let resp = self.inner.client.get(url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Error::Http(format!("HTTP {status} {url}")));
        }
        let text = resp.text().await?;
        serde_json::from_str(&text)
            .map_err(|err| Error::Parse(format!("JSON 解析失败（{url}）：{err}")))
    }

    /// 网络请求统计：(网络请求数, 缓存命中数, 最终失败数)。
    pub fn stats(&self) -> (u64, u64, u64) {
        (
            self.inner.network_requests.load(Ordering::Relaxed),
            self.inner.cache_hits.load(Ordering::Relaxed),
            self.inner.failures.load(Ordering::Relaxed),
        )
    }

    fn cache_path(&self, url: &str) -> Option<PathBuf> {
        self.inner
            .cache_dir
            .as_ref()
            .map(|dir| dir.join(format!("{:016x}.json", fnv1a(url))))
    }
}

/// FNV-1a 64bit：把 URL 稳定映射成缓存文件名（无第三方 hash 依赖）。
fn fnv1a(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn write_cache(path: &Path, value: &serde_json::Value) {
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let text = match serde_json::to_string(value) {
        Ok(text) => text,
        Err(_) => return,
    };
    let _ = std::fs::write(path, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_stable_and_distinct() {
        assert_eq!(fnv1a("https://a"), fnv1a("https://a"));
        assert_ne!(fnv1a("https://a"), fnv1a("https://b"));
    }

    #[test]
    fn client_defaults() {
        let client = HttpClient::new(15, None, false, false).expect("客户端构建失败");
        let (network, hits, failures) = client.stats();
        assert_eq!((network, hits, failures), (0, 0, 0));
    }
}
