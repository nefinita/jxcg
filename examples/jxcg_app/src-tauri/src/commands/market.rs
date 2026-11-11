//! 行情命令：本地价格读取/手动覆盖/联网刷新（JX3BOX）。
//!
//! 刷新流程（P3）：读本地 last-known-good → 并发拉 JX3BOX → 成功覆盖缓存、失败回落旧价 →
//! 写回 `应用数据目录/hangqing/{服务器}.toml`，通过 `refresh-progress` 事件回报进度。

use std::path::PathBuf;

use jxcg_api::{JiageJilu, RefreshProgress, RefreshResult, Tong};
use jxcg_market::{Jx3BoxProvider, ProviderConfig};
use tauri::{Emitter, Manager};

use crate::state::AppState;

fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|err| format!("获取应用数据目录失败：{err}"))
}

/// 读取本地行情档案（无档案返回空列表）。
#[tauri::command]
pub fn load_prices(app: tauri::AppHandle, fuwuqi_id: String) -> Result<Vec<JiageJilu>, String> {
    let dir = app_data_dir(&app)?;
    jxcg_market::load_prices(&dir, &fuwuqi_id).map_err(|err| err.to_string())
}

/// 手动覆盖一条物品价格（用户手动价优先于自动价，刷新时保留）。
#[tauri::command]
pub fn upsert_price(
    app: tauri::AppHandle,
    fuwuqi_id: String,
    wupin_id: String,
    danjia: Tong,
) -> Result<(), String> {
    let dir = app_data_dir(&app)?;
    jxcg_market::upsert_price(&dir, &fuwuqi_id, &wupin_id, danjia).map_err(|err| err.to_string())
}

/// 联网刷新行情；失败自动回落最后一次拉取价（last-known-good）。
#[tauri::command]
pub async fn refresh_prices(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    fuwuqi_id: String,
    wupin_ids: Vec<String>,
) -> Result<RefreshResult, String> {
    let catalog = state.catalog()?;
    let fuwuqi = catalog
        .fuwuqi
        .iter()
        .find(|fuwuqi| fuwuqi.id == fuwuqi_id)
        .ok_or_else(|| format!("未知服务器：{fuwuqi_id}"))?;
    let fuwuqi_mingcheng = fuwuqi.mingcheng.clone();
    let dir = app_data_dir(&app)?;
    let provider = Jx3BoxProvider::new(ProviderConfig::default())
        .map_err(|err| format!("行情客户端初始化失败：{err}"))?;
    let app_for_progress = app.clone();
    let outcome = jxcg_market::refresh_prices(
        &provider,
        &dir,
        &fuwuqi_id,
        &fuwuqi_mingcheng,
        &wupin_ids,
        move |progress: RefreshProgress| {
            let _ = app_for_progress.emit("refresh-progress", progress);
        },
    )
    .await
    .map_err(|err| err.to_string())?;
    Ok(outcome.into_result())
}
