//! 统一错误类型：库代码不 panic，全部走 Result。

use std::fmt;

/// JXCG 核心错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JxcgError {
    /// 金额解析错误
    Money(String),
    /// 数据缺失/非法（配方不存在、缺价等）
    Data(String),
    /// IO 错误
    Io(String),
    /// TOML 解析错误
    Toml(String),
    /// 网络错误（market 层用）
    Network(String),
    /// 暂不支持
    Unsupported(String),
}

impl fmt::Display for JxcgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JxcgError::Money(msg) => write!(f, "金额错误：{msg}"),
            JxcgError::Data(msg) => write!(f, "数据错误：{msg}"),
            JxcgError::Io(msg) => write!(f, "IO 错误：{msg}"),
            JxcgError::Toml(msg) => write!(f, "TOML 解析错误：{msg}"),
            JxcgError::Network(msg) => write!(f, "网络错误：{msg}"),
            JxcgError::Unsupported(msg) => write!(f, "暂不支持：{msg}"),
        }
    }
}

impl std::error::Error for JxcgError {}

impl From<std::io::Error> for JxcgError {
    fn from(value: std::io::Error) -> Self {
        JxcgError::Io(value.to_string())
    }
}

impl From<toml::de::Error> for JxcgError {
    fn from(value: toml::de::Error) -> Self {
        JxcgError::Toml(value.to_string())
    }
}

/// 核心 Result 别名。
pub type Result<T> = std::result::Result<T, JxcgError>;
