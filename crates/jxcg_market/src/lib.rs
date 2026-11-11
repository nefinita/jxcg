// ============================================================================
// jxcg_market —— 行情数据源（v0：JX3BOX 交易行接口）+ 本地行情缓存/降级
//
// 分层：
// - `jx3box`：HTTP 客户端（`next2.jx3box.com/api/item-price/{id}/detail?server=`），
//   超时 15s / 并发 4 / 失败重试 1 次 / UA `jxcg/0.1`；
// - `cache`：用户数据 `hangqing/{服务器}.toml` 读写（每条物品最后挂单 + 更新时间 + 来源）；
// - `refresh`：刷新汇总（成功/失败/缺价）+ last-known-good 降级（失败回落缓存价）。
//
// 约定：HTTP 只在本 crate；金额一律 i64 铜；失败不 panic，统一 Result。
// ============================================================================

pub mod cache;
pub mod error;
pub mod jx3box;
pub mod refresh;

pub use cache::{
    HangqingDangan, cache_path, hangqing_dir, jiage_map, load_dangan, load_prices, save_dangan,
    save_prices, upsert_price, wupin_sort_key, zuidi_danjia,
};
pub use error::{MarketError, Result};
pub use jx3box::{Jx3BoxProvider, ProviderConfig};
pub use refresh::{RefreshOutcome, XingqingJieguo, fetch_xingqing, merge_results, refresh_prices};

/// 当前 Unix 时间戳（秒）；刷新/测试都走这里，方便注入。
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
