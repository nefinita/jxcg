//! 设置命令：本地存档（P1 占位，P3 落盘到应用数据目录）。

use jxcg_api::Settings;

/// 读取设置（默认值）。
#[tauri::command]
pub fn load_settings() -> Result<Settings, String> {
    Ok(Settings::default())
}

/// 保存设置（P1：占位）。
#[tauri::command]
pub fn save_settings(_settings: Settings) -> Result<(), String> {
    Err("设置持久化待 P3 实现".to_string())
}
