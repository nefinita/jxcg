//! 设置命令：本地存档（应用数据目录 `settings.toml`，不进仓库）。
//!
//! 约定：读不到/没有文件 → 返回默认设置；写入失败给中文错误提示。

use std::fs;
use std::path::PathBuf;

use jxcg_api::Settings;
use tauri::Manager;

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("获取应用数据目录失败：{err}"))?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建配置目录失败：{err}"))?;
    Ok(dir.join("settings.toml"))
}

/// 读取设置；文件不存在时返回默认值。
#[tauri::command]
pub fn load_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    let path = settings_path(&app)?;
    if !path.is_file() {
        return Ok(Settings::default());
    }
    let text = fs::read_to_string(&path).map_err(|err| format!("读取设置失败：{err}"))?;
    toml::from_str(&text).map_err(|err| format!("解析设置失败：{err}"))
}

/// 保存设置到应用数据目录。
#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    let path = settings_path(&app)?;
    let text =
        toml::to_string_pretty(&settings).map_err(|err| format!("序列化设置失败：{err}"))?;
    fs::write(&path, text).map_err(|err| format!("写入设置失败：{err}"))
}
