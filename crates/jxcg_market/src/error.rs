//! 行情层错误。

use std::fmt;

#[derive(Debug, Clone)]
pub enum MarketError {
    /// HTTP / 网络错误
    Http(String),
    /// 响应 JSON 结构不符合预期
    Parse(String),
    /// 上游返回业务错误
    Upstream(String),
    /// 本地行情缓存（用户数据）读写/解析错误
    Cache(String),
    /// 未实现（占位）
    Unsupported(String),
}

impl fmt::Display for MarketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarketError::Http(msg) => write!(f, "网络请求失败：{msg}"),
            MarketError::Parse(msg) => write!(f, "行情响应解析失败：{msg}"),
            MarketError::Upstream(msg) => write!(f, "行情接口返回错误：{msg}"),
            MarketError::Cache(msg) => write!(f, "行情缓存错误：{msg}"),
            MarketError::Unsupported(msg) => write!(f, "暂不支持：{msg}"),
        }
    }
}

impl std::error::Error for MarketError {}

pub type Result<T> = std::result::Result<T, MarketError>;
