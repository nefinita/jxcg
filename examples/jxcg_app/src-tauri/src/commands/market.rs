//! 行情命令：本地价格读取/覆盖/刷新。
//!
//! P1 先给契约与空实现，P3 接入 jxcg_market（JX3BOX）+ 本地行情档案。

use jxcg_api::{JiageJilu, RefreshResult, Tong};

/// 读取本地行情（P1：空）。
#[tauri::command]
pub fn load_prices(_fuwuqi_id: String) -> Result<Vec<JiageJilu>, String> {
    Ok(Vec::new())
}

/// 手动覆盖一条物品价格（P1：占位）。
#[tauri::command]
pub fn upsert_price(
    _fuwuqi_id: String,
    _wupin_id: String,
    _danjia: Tong,
) -> Result<(), String> {
    Err("行情写入待 P3 实现".to_string())
}

/// 联网刷新行情（P1：占位；P3 接 JX3BOX + 并发/缓存/降级）。
#[tauri::command]
pub fn refresh_prices(
    fuwuqi_id: String,
    _wupin_ids: Vec<String>,
) -> Result<RefreshResult, String> {
    let _ = fuwuqi_id;
    Err("自动拉价待 P3 实现".to_string())
}
