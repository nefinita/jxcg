//! 精确整数取整工具。
//!
//! 金额、期望收入、比率全部用整数（i128 中间量 → i64 铜）计算，
//! 避免 f64 在大额或批量场景下出现累计误差；这里统一放取整口径。

use crate::{JxcgError, Result};
use jxcg_api::Tong;

/// 非负整数除法，HALF_UP（四舍五入）到整数。
///
/// `num >= 0`、`den > 0`；等价于 `floor(num / den + 0.5)`。
pub(crate) fn round_div_half_up(num: i128, den: i128) -> i128 {
    debug_assert!(num >= 0, "round_div_half_up 只接受非负数");
    debug_assert!(den > 0, "round_div_half_up 只接受正分母");
    (num + den / 2) / den
}

/// 有符号整数除法，HALF_UP（四舍五入，负数远离 0）到整数。
pub(crate) fn round_div_half_up_signed(num: i128, den: i128) -> i128 {
    debug_assert!(den > 0, "round_div_half_up_signed 只接受正分母");
    if num >= 0 {
        return round_div_half_up(num, den);
    }
    // 用 u128 取绝对值，避免 i128::MIN 取负溢出。
    let magnitude = num.unsigned_abs();
    let den_u = den as u128;
    let rounded = (magnitude + den_u / 2) / den_u;
    if rounded > i128::MAX as u128 {
        i128::MIN
    } else {
        -(rounded as i128)
    }
}

/// 非负整数除法，向上取整（`num >= 0`、`den > 0`）。
pub(crate) fn ceil_div(num: i128, den: i128) -> i128 {
    debug_assert!(num >= 0, "ceil_div 只接受非负数");
    debug_assert!(den > 0, "ceil_div 只接受正分母");
    if num == 0 { 0 } else { (num - 1) / den + 1 }
}

/// i128 乘法，溢出返回中文错误。
pub(crate) fn checked_mul(a: i128, b: i128, field: &str) -> Result<i128> {
    a.checked_mul(b)
        .ok_or_else(|| JxcgError::Data(format!("{field}超出可计算范围（溢出）")))
}

/// i128 加法，溢出返回中文错误。
pub(crate) fn checked_add(a: i128, b: i128, field: &str) -> Result<i128> {
    a.checked_add(b)
        .ok_or_else(|| JxcgError::Data(format!("{field}超出可计算范围（溢出）")))
}

/// i128 → i64 铜，超出范围返回中文错误。
pub(crate) fn to_tong(value: i128, field: &str) -> Result<Tong> {
    if value > i64::MAX as i128 || value < i64::MIN as i128 {
        return Err(JxcgError::Data(format!(
            "{field}超出金额范围（{} 铜）",
            value
        )));
    }
    Ok(value as Tong)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding() {
        assert_eq!(round_div_half_up(0, 3), 0);
        assert_eq!(round_div_half_up(1, 3), 0);
        assert_eq!(round_div_half_up(2, 3), 1);
        assert_eq!(round_div_half_up(1, 2), 1);
        assert_eq!(round_div_half_up(3, 2), 2);
        assert_eq!(round_div_half_up(149, 100), 1);
        assert_eq!(round_div_half_up(150, 100), 2);
        assert_eq!(round_div_half_up(250, 100), 3);
        // 负数远离 0 舍入
        assert_eq!(round_div_half_up_signed(-149, 100), -1);
        assert_eq!(round_div_half_up_signed(-150, 100), -2);
        assert_eq!(round_div_half_up_signed(-250, 100), -3);
        assert_eq!(round_div_half_up_signed(-1, 3), 0);
        assert_eq!(round_div_half_up_signed(-2, 3), -1);
        // 向上取整
        assert_eq!(ceil_div(0, 7), 0);
        assert_eq!(ceil_div(1, 7), 1);
        assert_eq!(ceil_div(7, 7), 1);
        assert_eq!(ceil_div(8, 7), 2);
    }

    #[test]
    fn checked_helpers() {
        assert_eq!(checked_mul(2, 3, "x").unwrap(), 6);
        assert!(checked_mul(i128::MAX, 2, "x").is_err());
        assert_eq!(checked_add(2, 3, "x").unwrap(), 5);
        assert!(checked_add(i128::MAX, 1, "x").is_err());
        assert_eq!(to_tong(42, "x").unwrap(), 42);
        assert!(to_tong(i64::MAX as i128 + 1, "x").is_err());
        assert!(to_tong(i64::MIN as i128 - 1, "x").is_err());
    }
}
