//! 数据命令：加载随包目录（物品/配方/服务器/值集）。

use jxcg_api::Catalog;

use crate::state::AppState;

/// 加载 data/ 下的完整目录；数据目录可用 JXCG_DATA_DIR 覆盖（进程内缓存）。
#[tauri::command]
pub fn load_catalog(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<Catalog, String> {
    state.catalog(&app).map(|catalog| (*catalog).clone())
}

/// 清缓存并重新读取 data/（数据重新导出后使用）。
#[tauri::command]
pub fn reload_catalog(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<Catalog, String> {
    state.clear();
    state.catalog(&app).map(|catalog| (*catalog).clone())
}
