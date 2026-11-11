// ============================================================================
// lib.rs — Tauri 应用装配：注册命令 + 启动窗口
// ============================================================================

mod commands;
mod state;

pub fn run() {
    let result = tauri::Builder::default()
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::calc::calculate_net,
            commands::calc::calculate_baoben,
            commands::calc::calculate_recipe,
            commands::calc::rank_recipes,
            commands::calc::calculate_convert,
            commands::data::load_catalog,
            commands::data::reload_catalog,
            commands::market::load_prices,
            commands::market::upsert_price,
            commands::market::refresh_prices,
            commands::settings::load_settings,
            commands::settings::save_settings,
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("JXCG 启动失败：{err}");
    }
}
