//! 导出工具的错误类型（bin 工具，不引入额外依赖）。

use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// 文件读写
    Io(String),
    /// 网络请求
    Http(String),
    /// JSON/TOML 解析
    Parse(String),
    /// 数据不符合预期
    Data(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(msg) => write!(f, "IO 错误：{msg}"),
            Error::Http(msg) => write!(f, "网络错误：{msg}"),
            Error::Parse(msg) => write!(f, "解析错误：{msg}"),
            Error::Data(msg) => write!(f, "数据错误：{msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Http(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Parse(err.to_string())
    }
}

impl From<toml::ser::Error> for Error {
    fn from(err: toml::ser::Error) -> Self {
        Error::Parse(err.to_string())
    }
}
