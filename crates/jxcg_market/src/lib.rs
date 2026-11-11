// ============================================================================
// jxcg_market —— 行情数据源（v0：JX3BOX 交易行接口）
//
// 设计：core 只管算，不联网；所有 HTTP 收敛在这里。
// - Jx3BoxProvider：`next2.jx3box.com/api/item-price/{id}/detail?server=`
// - 失败/超时由上层降级到最后一次缓存价（last-known-good），本层只返回错误。
// ============================================================================

pub mod error;
pub mod jx3box;

pub use error::MarketError;
pub use jx3box::{Jx3BoxProvider, ProviderConfig};
