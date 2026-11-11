//! 金 / 银 / 铜 解析与格式化。
//!
//! - 1 金 = 100 银 = 10_000 铜；1 砖 = 10_000 金 = 100_000_000 铜；
//! - 内部整数铜运算；输入解析兼容「123金45银67铜」「123.4567金」「12345」等。

use crate::{JxcgError, Result};
use jxcg_api::Tong;

const TONG_PER_YIN: i128 = 100;
const TONG_PER_JIN: i128 = 10_000;
const TONG_PER_ZHUAN: i128 = 100_000_000;

/// 解析金额文本，返回铜。
///
/// 规则：
/// - 带单位：`1砖2金3银4铜`，单位可缺省但顺序任意；数字可带小数（精度受单位限制：金最多 4 位）；
/// - 不带单位：纯数字按「金」处理，`123.45` = 123 金 45 银；
/// - 空串、负数、非法字符、精度超限一律报错。
pub fn parse_jin(input: &str) -> Result<Tong> {
    let s: String = input.trim().chars().filter(|c| !matches!(c, ' ' | ',' | '，')).collect();
    if s.is_empty() {
        return Err(JxcgError::Money("金额为空".into()));
    }
    if s.starts_with('-') {
        return Err(JxcgError::Money("金额不能为负".into()));
    }

    let has_unit = s.chars().any(|c| matches!(c, '砖' | '金' | '银' | '铜'));
    let mut total: i128 = 0;
    if has_unit {
        let mut num = String::new();
        let mut saw_unit = false;
        for ch in s.chars() {
            let unit = match ch {
                '砖' => Some(TONG_PER_ZHUAN),
                '金' => Some(TONG_PER_JIN),
                '银' => Some(TONG_PER_YIN),
                '铜' => Some(1_i128),
                _ => None,
            };
            match unit {
                Some(unit) => {
                    if num.is_empty() {
                        return Err(JxcgError::Money(format!("单位「{ch}」前缺少数字")));
                    }
                    total = total
                        .checked_add(parse_amount(&num, unit)?)
                        .ok_or_else(|| JxcgError::Money("金额溢出".into()))?;
                    num.clear();
                    saw_unit = true;
                }
                None => {
                    if ch.is_ascii_digit() || ch == '.' {
                        num.push(ch);
                    } else {
                        return Err(JxcgError::Money(format!("无法识别的字符「{ch}」")));
                    }
                }
            }
        }
        if !num.is_empty() {
            return Err(JxcgError::Money("末尾数字缺少单位（砖/金/银/铜）".into()));
        }
        if !saw_unit {
            return Err(JxcgError::Money("未找到金额单位".into()));
        }
    } else {
        total = parse_amount(&s, TONG_PER_JIN)?;
    }

    if total > i64::MAX as i128 {
        return Err(JxcgError::Money("金额超出范围".into()));
    }
    Ok(total as Tong)
}

/// 按单位解析一个数字段（可能带小数），返回铜。
fn parse_amount(text: &str, unit: i128) -> Result<i128> {
    if text.is_empty() {
        return Err(JxcgError::Money("金额为空".into()));
    }
    let (int_part, frac_part) = match text.split_once('.') {
        Some((a, b)) => (a, b),
        None => (text, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(JxcgError::Money("金额为空".into()));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) || !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(JxcgError::Money(format!("非法数字「{text}」")));
    }
    // 单位对应的可接受小数位数：砖 8 位、金 4 位、银 2 位、铜 0 位。
    let max_frac = match unit {
        TONG_PER_ZHUAN => 8,
        TONG_PER_JIN => 4,
        TONG_PER_YIN => 2,
        _ => 0,
    };
    if frac_part.len() > max_frac {
        return Err(JxcgError::Money(format!(
            "小数位数超出精度（该单位最多 {max_frac} 位）"
        )));
    }
    let int_value: i128 = if int_part.is_empty() {
        0
    } else {
        int_part
            .parse::<i128>()
            .map_err(|_| JxcgError::Money(format!("非法数字「{text}」")))?
    };
    let mut value = int_value
        .checked_mul(unit)
        .ok_or_else(|| JxcgError::Money("金额溢出".into()))?;
    if !frac_part.is_empty() {
        let frac_value: i128 = frac_part
            .parse::<i128>()
            .map_err(|_| JxcgError::Money(format!("非法数字「{text}」")))?;
        let scale = 10_i128.pow(frac_part.len() as u32);
        let frac_tong = frac_value
            .checked_mul(unit)
            .and_then(|v| v.checked_div(scale))
            .ok_or_else(|| JxcgError::Money("金额溢出".into()))?;
        // 余数非零说明精度不够（例如 0.5 铜），直接报错。
        if frac_value * unit % scale != 0 {
            return Err(JxcgError::Money(format!("金额精度超过 1 铜「{text}」")));
        }
        value = value
            .checked_add(frac_tong)
            .ok_or_else(|| JxcgError::Money("金额溢出".into()))?;
    }
    Ok(value)
}

/// 格式化铜为「1砖2金3银4铜」（0 项省略；负数加「负」前缀）。
pub fn format_tong(v: Tong) -> String {
    if v == 0 {
        return "0铜".to_string();
    }
    let negative = v < 0;
    let abs = v.unsigned_abs();
    let units: [(u64, &str); 4] = [
        (100_000_000, "砖"),
        (10_000, "金"),
        (100, "银"),
        (1, "铜"),
    ];
    let mut rest = abs;
    let mut parts: Vec<String> = Vec::new();
    for (unit, name) in units {
        let count = rest / unit;
        rest %= unit;
        if count > 0 {
            parts.push(format!("{count}{name}"));
        }
    }
    if parts.is_empty() {
        parts.push("0铜".to_string());
    }
    if negative {
        format!("负{}", parts.concat())
    } else {
        parts.concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_units() {
        assert_eq!(parse_jin("1金").unwrap(), 10_000);
        assert_eq!(parse_jin("1金20银3铜").unwrap(), 12_003);
        assert_eq!(parse_jin("1砖").unwrap(), 100_000_000);
        assert_eq!(parse_jin("1砖2金").unwrap(), 100_020_000);
        assert_eq!(parse_jin(" 1,234 金 ").unwrap(), 12_340_000);
    }

    #[test]
    fn parse_decimal() {
        assert_eq!(parse_jin("123.45").unwrap(), 1_234_500);
        assert_eq!(parse_jin("123.4567").unwrap(), 1_234_567);
        assert_eq!(parse_jin("0.01").unwrap(), 100);
        assert!(parse_jin("0.001").is_ok());
        assert!(parse_jin("1.23456").is_err());
        assert!(parse_jin("12345").is_ok());
        assert_eq!(parse_jin("12345").unwrap(), 123_450_000);
    }

    #[test]
    fn parse_errors() {
        assert!(parse_jin("").is_err());
        assert!(parse_jin("-1金").is_err());
        assert!(parse_jin("1金2").is_err());
        assert!(parse_jin("1g").is_err());
        assert!(parse_jin("金").is_err());
    }

    #[test]
    fn format_roundtrip() {
        assert_eq!(format_tong(0), "0铜");
        assert_eq!(format_tong(12_003), "1金20银3铜");
        assert_eq!(format_tong(10_000), "1金");
        assert_eq!(format_tong(100_000_000), "1砖");
        assert_eq!(format_tong(-12_003), "负1金20银3铜");
        for v in [0_i64, 1, 99, 100, 10_000, 12_345, 100_000_000, 123_456_789] {
            let text = format_tong(v);
            assert_eq!(parse_jin(&text).unwrap(), v, "roundtrip {text}");
        }
    }
}
