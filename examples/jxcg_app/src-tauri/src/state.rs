//! 应用共享状态：随包目录的进程内缓存。
//!
//! 目录数据（物品/配方/服务器/值集）在启动后基本不变，缓存为 `Arc<Catalog>` 可避免每次命令重复解析 TOML；
//! 数据重新导出后可调用 `reload_catalog` 清缓存重载。

use std::sync::{Arc, RwLock};

use jxcg_api::Catalog;
use jxcg_core::store;

#[derive(Default)]
pub struct AppState {
    catalog: RwLock<Option<Arc<Catalog>>>,
}

impl AppState {
    /// 取目录缓存；未加载则从 data/ 加载并缓存。
    pub fn catalog(&self) -> Result<Arc<Catalog>, String> {
        {
            let guard = self
                .catalog
                .read()
                .map_err(|_| "目录缓存读取锁损坏".to_string())?;
            if let Some(catalog) = guard.as_ref() {
                return Ok(catalog.clone());
            }
        }
        let (_, loaded) = store::load_from_default_dir().map_err(|err| err.to_string())?;
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
