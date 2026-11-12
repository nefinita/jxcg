// ============================================================================
// jxcg_core —— 纯计算核心（不联网、不依赖 Tauri）
//
// 分层：money（金银铜）→ fee（手续费/保管费/保本价）→ craft（配方成本利润）
//       → rank（排行）/ convert（五行石转换）→ store（读 data/ TOML）
// 内部工具：math（整数取整/溢出检查，金额全程 i128 中间量 → i64 铜）
//
// 约定：
// - 金额一律 i64「铜」，不用浮点表示金额；概率/期望/占比展示可以用 f64；
// - 库代码禁止 unwrap/expect（沿用 jpcg 教训），统一 Result<_, JxcgError>；
// - 所有口径先在单测里钉死（金标准），再给 UI 用。
// ============================================================================

pub mod convert;
pub mod craft;
pub mod error;
pub mod fee;
mod math;
pub mod money;
pub mod rank;
pub mod store;
mod zhongjian;

pub use error::{JxcgError, Result};
