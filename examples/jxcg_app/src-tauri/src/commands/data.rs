//! 数据命令：加载随包目录（物品/配方/服务器/值集）。

use jxcg_api::Catalog;
use jxcg_core::store;

/// 加载 data/ 下的完整目录；数据目录可用 JXCG_DATA_DIR 覆盖。
#[tauri::command]
pub fn load_catalog() -> Result<Catalog, String> {
    store::load_from_default_dir()
        .map(|(_, catalog)| catalog)
        .map_err(|err| err.to_string())
}
