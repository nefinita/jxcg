//! 应用共享状态：随包目录的进程内缓存 + 数据目录解析。
//!
//! 数据目录解析顺序：
//! 1. 环境变量 `JXCG_DATA_DIR`（显式指定；无效时报错，不静默回落）
//! 2. Tauri 资源目录 `resource_dir()/data`（打包后的 .app / 安装包）
//! 3. `jxcg_core::store::resolve_data_dir()`（exe 同目录 data/；debug 下回退仓库 data/）
//!
//! 目录数据（物品/配方/服务器/值集）在启动后基本不变，缓存为 `Arc<Catalog>` 避免命令重复解析 TOML；
//! 数据重新导出后可调用 `reload_catalog` 清缓存重载。

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use jxcg_api::Catalog;
use jxcg_core::store;
use tauri::Manager;

#[derive(Default)]
pub struct AppState {
    catalog: RwLock<Option<Arc<Catalog>>>,
}

impl AppState {
    /// 取目录缓存；未加载则解析数据目录并加载。
    pub fn catalog(&self, app: &tauri::AppHandle) -> Result<Arc<Catalog>, String> {
        {
            let guard = self
                .catalog
                .read()
                .map_err(|_| "目录缓存读取锁损坏".to_string())?;
            if let Some(catalog) = guard.as_ref() {
                return Ok(catalog.clone());
            }
        }
        let dir = resolve_data_dir(app)?;
        let loaded = store::load_catalog(&dir)
            .map_err(|err| format!("加载数据目录 {} 失败：{err}", dir.display()))?;
        let catalog = Arc::new(loaded);
        let mut guard = self
            .catalog
            .write()
            .map_err(|_| "目录缓存写入锁损坏".to_string())?;
        *guard = Some(catalog.clone());
        Ok(catalog)
    }

    /// 清空缓存，下次取用时重新读取 data/。
    pub fn clear(&self) {
        if let Ok(mut guard) = self.catalog.write() {
            *guard = None;
        }
    }
}

/// 解析随包数据目录（见模块注释的优先级）。
pub fn resolve_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Ok(value) = std::env::var("JXCG_DATA_DIR")
        && !value.trim().is_empty()
    {
        let path = PathBuf::from(value);
        if path.is_dir() {
            return Ok(path);
        }
        return Err(format!("JXCG_DATA_DIR 指向的目录不存在：{}", path.display()));
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join("data");
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    store::resolve_data_dir()
        .ok_or_else(|| "找不到 data/ 目录（打包资源缺失，可用 JXCG_DATA_DIR 指定）".to_string())
}
