//! 随包数据读取：data/{catalog,wupin,peifang,wuxingshi,fuwuqi,values}.toml。
//!
//! 数据目录解析顺序：
//! 1. 环境变量 `JXCG_DATA_DIR`
//! 2. 可执行文件同目录下的 `data/`
//! 3. debug 构建下回退到仓库内 `data/`（开发期方便，release 不启用）

use std::path::{Path, PathBuf};

use crate::{JxcgError, Result};
use jxcg_api::{Banben, Catalog, Fuwuqi, Peifang, Wupin, Zhiji, Zhuanhuan};
use serde::de::DeserializeOwned;

#[derive(serde::Deserialize)]
struct CatalogFile {
    banben: Banben,
}

#[derive(serde::Deserialize, Default)]
struct WupinFile {
    #[serde(default)]
    wupin: Vec<Wupin>,
}

#[derive(serde::Deserialize, Default)]
struct PeifangFile {
    #[serde(default)]
    peifang: Vec<Peifang>,
}

#[derive(serde::Deserialize, Default)]
struct WuxingshiFile {
    #[serde(default)]
    zhuanhuan: Vec<Zhuanhuan>,
}

#[derive(serde::Deserialize, Default)]
struct FuwuqiFile {
    #[serde(default)]
    fuwuqi: Vec<Fuwuqi>,
}

#[derive(serde::Deserialize, Default)]
struct ValuesFile {
    #[serde(default)]
    zhiji: Vec<Zhiji>,
}

fn read_toml<T: DeserializeOwned>(dir: &Path, name: &str) -> Result<T> {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).map_err(|err| {
        JxcgError::Io(format!("读取 {} 失败：{err}", path.display()))
    })?;
    toml::from_str(&text).map_err(|err| {
        JxcgError::Toml(format!("解析 {} 失败：{err}", path.display()))
    })
}

/// 从指定目录加载完整目录数据。
pub fn load_catalog(dir: &Path) -> Result<Catalog> {
    let catalog_file: CatalogFile = read_toml(dir, "catalog.toml")?;
    let wupin: WupinFile = read_toml(dir, "wupin.toml")?;
    let peifang: PeifangFile = read_toml(dir, "peifang.toml")?;
    let wuxingshi: WuxingshiFile = read_toml(dir, "wuxingshi.toml")?;
    let fuwuqi: FuwuqiFile = read_toml(dir, "fuwuqi.toml")?;
    let values: ValuesFile = read_toml(dir, "values.toml")?;
    Ok(Catalog {
        banben: catalog_file.banben,
        wupin: wupin.wupin,
        peifang: peifang.peifang,
        wuxingshi: wuxingshi.zhuanhuan,
        fuwuqi: fuwuqi.fuwuqi,
        zhiji: values.zhiji,
    })
}

/// 解析数据目录。
pub fn resolve_data_dir() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("JXCG_DATA_DIR")
        && !value.trim().is_empty()
    {
        let path = PathBuf::from(value);
        if path.is_dir() {
            return Some(path);
        }
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let candidate = parent.join("data");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    #[cfg(debug_assertions)]
    {
        let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

/// 解析并加载数据目录（解析失败给中文错误）。
pub fn load_from_default_dir() -> Result<(PathBuf, Catalog)> {
    let dir = resolve_data_dir()
        .ok_or_else(|| JxcgError::Data("找不到 data/ 目录（可用 JXCG_DATA_DIR 指定）".into()))?;
    let catalog = load_catalog(&dir)?;
    Ok((dir, catalog))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_repo_data() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let catalog = load_catalog(&dir).expect("data/ 应能加载");
        assert!(!catalog.wupin.is_empty());
        assert!(!catalog.peifang.is_empty());
        assert!(!catalog.zhiji.is_empty());
    }
}
