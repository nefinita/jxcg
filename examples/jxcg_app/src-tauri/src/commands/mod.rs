//! Tauri 命令：按域拆分（calc / data / market / settings），与前端 `api/commands.ts` 一一对应。

pub mod calc;
pub mod data;
pub mod market;
pub mod settings;

/// 冒烟命令：确认前后端通道正常。
#[tauri::command]
pub fn ping() -> String {
    "pong".to_string()
}
