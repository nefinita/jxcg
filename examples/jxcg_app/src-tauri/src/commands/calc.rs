//! 计算命令：净收入/保本价/单配方/排行/五行石转换。

use jxcg_api::{
    BaobenRequest, JisuanRequest, JisuanResult, NetRequest, NetResult, PaihangRequest, PaihangRow,
    Tong, ZhuanhuanRequest, ZhuanhuanResult,
};
use jxcg_core::{convert, craft, fee, rank};

use crate::state::AppState;

/// 交易行净收入（扣 5% 手续费 + 保管费）。
#[tauri::command]
pub fn calculate_net(req: NetRequest) -> Result<NetResult, String> {
    fee::jisuan_net(&req).map_err(|err| err.to_string())
}

/// 保本总售价。
#[tauri::command]
pub fn calculate_baoben(req: BaobenRequest) -> Result<Tong, String> {
    fee::baobenjia(req.chengben, &req.shouxufei, req.guadan_cishu).map_err(|err| err.to_string())
}

/// 单配方成本利润。
#[tauri::command]
pub fn calculate_recipe(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    req: JisuanRequest,
) -> Result<JisuanResult, String> {
    let catalog = state.catalog(&app)?;
    craft::jisuan(&catalog, &req).map_err(|err| err.to_string())
}

/// 全配方排行。
#[tauri::command]
pub fn rank_recipes(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    req: PaihangRequest,
) -> Result<Vec<PaihangRow>, String> {
    let catalog = state.catalog(&app)?;
    Ok(rank::paihang(&catalog, &req))
}

/// 五行石转换（精炼）。
#[tauri::command]
pub fn calculate_convert(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    req: ZhuanhuanRequest,
) -> Result<ZhuanhuanResult, String> {
    let catalog = state.catalog(&app)?;
    convert::jisuan(&catalog, &req).map_err(|err| err.to_string())
}
