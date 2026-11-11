//! 金 / 银 / 铜 解析与格式化。
//!
//! - 1 金 = 100 银 = 10_000 铜；1 砖 = 10_000 金 = 100_000_000 铜；
//! - 内部整数铜运算（[`Tong`] = `i64`），解析结果可以为负（用于展示负利润）；
//!   售价、成本、单价等**业务字段是否允许负数由各计算模块单独校验**；
//! - 输入解析兼容「123金45银67铜」「123.4567金」「12345」「-1金」「负1金20银3铜」等；
//! - 输出格式固定为「1砖2金3银4铜」，0 项省略、负数加「负」前缀，
//!   并且对**全部 i64 取值**（含 [`i64::MIN`]）与 [`parse_jin`] 往返一致。

use crate::{JxcgError, Result};
use jxcg_api::Tong;

const TONG_PER_YIN: i128 = 100;
const TONG_PER_JIN: i128 = 10_000;
const TONG_PER_ZHUAN: i128 = 100_000_000;

/// 负数允许的最大绝对值：`i64::MIN` 比 `i64::MAX` 多 1 铜。
const MAX_TONG_ABS: i128 = i64::MAX as i128 + 1;

/// 解析金额文本，返回铜（可带负号）。
///
/// 规则：
/// - 可带正负号：`-1金`、`负1金`；不带符号按正数处理；
/// - 带单位：`1砖2金3银4铜`，单位顺序不限；数字可带小数（精度受单位限制：
///   砖最多 8 位、金最多 4 位、银最多 2 位、铜不允许小数）；
/// - 不带单位：纯数字按「金」处理，`123.45` = 123 金 45 银；
/// - 忽略空白与千分位分隔符（`,` 和 `，`）；
/// - 空串、符号重复、非法字符、超出精度、超出 i64 范围一律报中文错误。
pub fn parse_jin(input: &str) -> Result<Tong> {
    let s: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, ',' | '，'))
        .collect();
    if s.is_empty() {
        return Err(JxcgError::Money("金额为空".into()));
    }

    let (negative, body) = split_sign(&s)?;
    let magnitude = if body.chars().any(is_unit) {
        parse_with_units(&body)?
    } else {
        // 不带单位：纯数字默认按「金」
        parse_amount(&body, TONG_PER_JIN)?
    };
    signed_tong(magnitude, negative)
}

/// 拆分正负号，返回（是否负数，去掉符号后的正文）。
fn split_sign(text: &str) -> Result<(bool, String)> {
    let mut rest = text;
    let negative = if let Some(r) = rest.strip_prefix('负') {
        rest = r;
        true
    } else if let Some(r) = rest.strip_prefix('-') {
        rest = r;
        true
    } else {
        false
    };
    if rest.is_empty() {
        return Err(JxcgError::Money("金额为空".into()));
    }
    // 只允许一个符号在最前；`--1`、`负-1`、`+-1` 都是非法输入。
    if rest.starts_with('负') || rest.starts_with('-') || rest.starts_with('+') {
        return Err(JxcgError::Money("金额符号重复或非法".into()));
    }
    Ok((negative, rest.to_string()))
}

fn is_unit(ch: char) -> bool {
    unit_of(ch).is_some()
}

fn unit_of(ch: char) -> Option<i128> {
    match ch {
        '砖' => Some(TONG_PER_ZHUAN),
        '金' => Some(TONG_PER_JIN),
        '银' => Some(TONG_PER_YIN),
        '铜' => Some(1),
        _ => None,
    }
}

/// 解析「数字+单位」串（如 `1砖2金3银4铜`），各段按单位累加。
fn parse_with_units(text: &str) -> Result<i128> {
    let mut total: i128 = 0;
    let mut num = String::new();
    let mut saw_unit = false;
    for ch in text.chars() {
        match unit_of(ch) {
            Some(unit) => {
                if num.is_empty() {
                    return Err(JxcgError::Money(format!("单位「{ch}」前缺少数字")));
                }
                let value = parse_amount(&num, unit)?;
                total = total
                    .checked_add(value)
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
        // 理论不可达（调用前已判断含单位），保留防御分支。
        return Err(JxcgError::Money("未找到金额单位".into()));
    }
    Ok(total)
}

/// 按单位解析一个数字段（可带小数），返回正数铜值。
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
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(JxcgError::Money(format!("非法数字「{text}」")));
    }
    // 单位对应的可接受小数位数：砖 8 位、金 4 位、银 2 位、铜 0 位。
    let max_frac = if unit == TONG_PER_ZHUAN {
        8
    } else if unit == TONG_PER_JIN {
        4
    } else if unit == TONG_PER_YIN {
        2
    } else {
        0
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
            .map_err(|_| JxcgError::Money(format!("非法或过大数字「{text}」")))?
    };
    let mut value = int_value
        .checked_mul(unit)
        .ok_or_else(|| JxcgError::Money("金额溢出".into()))?;
    if !frac_part.is_empty() {
        let frac_value: i128 = frac_part
            .parse::<i128>()
            .map_err(|_| JxcgError::Money(format!("非法数字「{text}」")))?;
        let scale = 10_i128.pow(frac_part.len() as u32);
        // 余数非零说明精度不够（例如 0.5 铜），直接报错而不是悄悄截断。
        let frac_checked = frac_value
            .checked_mul(unit)
            .ok_or_else(|| JxcgError::Money("金额溢出".into()))?;
        if frac_checked % scale != 0 {
            return Err(JxcgError::Money(format!("金额精度超过 1 铜「{text}」")));
        }
        let frac_tong = frac_checked / scale;
        value = value
            .checked_add(frac_tong)
            .ok_or_else(|| JxcgError::Money("金额溢出".into()))?;
    }
    Ok(value)
}

/// 把「正数绝对值 + 符号」转换为 i64 铜，覆盖完整的 i64 取值范围。
fn signed_tong(magnitude: i128, negative: bool) -> Result<Tong> {
    if magnitude < 0 {
        return Err(JxcgError::Money("金额内部错误（负绝对值）".into()));
    }
    if !negative {
        if magnitude > i64::MAX as i128 {
            return Err(JxcgError::Money("金额超出范围".into()));
        }
        return Ok(magnitude as Tong);
    }
    if magnitude > MAX_TONG_ABS {
        return Err(JxcgError::Money("金额超出范围".into()));
    }
    if magnitude == MAX_TONG_ABS {
        return Ok(i64::MIN);
    }
    Ok(-(magnitude as Tong))
}

/// 格式化铜为「1砖2金3银4铜」（0 项省略；负数加「负」前缀）。
///
/// 与 [`parse_jin`] 对全部 i64 取值往返一致（含 `i64::MIN` / `i64::MAX`）。
pub fn format_tong(v: Tong) -> String {
    if v == 0 {
        return "0铜".to_string();
    }
    let negative = v < 0;
    let mut rest: u64 = v.unsigned_abs();
    let units: [(u64, &str); 4] = [(100_000_000, "砖"), (10_000, "金"), (100, "银"), (1, "铜")];
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
    let body = parts.concat();
    if negative { format!("负{body}") } else { body }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_units() {
        assert_eq!(parse_jin("0").unwrap(), 0);
        assert_eq!(parse_jin("0铜").unwrap(), 0);
        assert_eq!(parse_jin("1铜").unwrap(), 1);
        assert_eq!(parse_jin("1银").unwrap(), 100);
        assert_eq!(parse_jin("1金").unwrap(), 10_000);
        assert_eq!(parse_jin("1砖").unwrap(), 100_000_000);
        assert_eq!(parse_jin("1砖2金").unwrap(), 100_020_000);
        assert_eq!(parse_jin("1金20银3铜").unwrap(), 12_003);
        // 单位顺序不限、千分位与空白忽略
        assert_eq!(parse_jin("3铜2银1金").unwrap(), 10_203);
        assert_eq!(parse_jin(" 1,234 金 ").unwrap(), 12_340_000);
        assert_eq!(parse_jin("1，234金").unwrap(), 12_340_000);
        assert_eq!(parse_jin("1 砖 0 金 0 银 1 铜").unwrap(), 100_000_001);
        // 省略 0 项
        assert_eq!(parse_jin("0砖1金").unwrap(), 10_000);
    }

    #[test]
    fn parse_decimal() {
        // 不带单位 → 按金
        assert_eq!(parse_jin("123.45").unwrap(), 1_234_500);
        assert_eq!(parse_jin("123.4567").unwrap(), 1_234_567);
        assert_eq!(parse_jin("0.01").unwrap(), 100);
        assert_eq!(parse_jin("0.0001").unwrap(), 1);
        assert_eq!(parse_jin("12345").unwrap(), 123_450_000);
        assert_eq!(parse_jin(".5").unwrap(), 5_000);
        // 带单位的小数
        assert_eq!(parse_jin("1.5金").unwrap(), 15_000);
        assert_eq!(parse_jin("1.05金").unwrap(), 10_500);
        assert_eq!(parse_jin("1.5银").unwrap(), 150);
        assert!(parse_jin("3.5铜").is_err(), "铜不允许小数");
        assert_eq!(parse_jin("0.05金").unwrap(), 500);
        assert_eq!(parse_jin("1.0500金").unwrap(), 10_500);
    }

    #[test]
    fn parse_negative() {
        assert_eq!(parse_jin("-1金").unwrap(), -10_000);
        assert_eq!(parse_jin("负1金20银3铜").unwrap(), -12_003);
        assert_eq!(parse_jin("-1砖").unwrap(), -100_000_000);
        // 负零归一为 0
        assert_eq!(parse_jin("-0金").unwrap(), 0);
        assert_eq!(parse_jin("负0铜").unwrap(), 0);
        // 符号非法
        for bad in ["-", "负", "--1金", "负-1金", "+-1金", "1金-2银", "负负1金"] {
            assert!(parse_jin(bad).is_err(), "应拒绝非法符号：{bad}");
        }
    }

    #[test]
    fn parse_precision_errors() {
        // 金最多 4 位小数（1 铜精度）；银 2 位；铜 0 位
        assert!(parse_jin("0.00001金").is_err());
        assert!(parse_jin("1.23456金").is_err());
        assert!(parse_jin("0.001银").is_err());
        assert!(parse_jin("1.5铜").is_err());
        assert!(parse_jin("0.000000001砖").is_err());
        // 恰好精度上限
        assert!(parse_jin("0.0001金").is_ok());
        assert!(parse_jin("0.01银").is_ok());
        assert!(parse_jin("0.00000001砖").is_ok());
    }

    #[test]
    fn parse_illegal() {
        for bad in [
            "", "   ", ",", "金", "银铜", "1金2", "1g", "abc", "1..2", "1.2.3", "1金.5", "-", "负",
            "1金-2",
        ] {
            assert!(parse_jin(bad).is_err(), "应拒绝非法输入：{bad:?}");
        }
    }

    #[test]
    fn parse_range() {
        assert_eq!(parse_jin("9223372036854775807铜").unwrap(), i64::MAX);
        assert!(parse_jin("9223372036854775808铜").is_err());
        // 负数可以取到 i64::MIN
        assert_eq!(parse_jin("-9223372036854775808铜").unwrap(), i64::MIN);
        assert!(parse_jin("-9223372036854775809铜").is_err());
        // 砖级超大输入（1e38 量级）不能 panic
        assert!(parse_jin("999999999999999999999999999999999999砖").is_err());
        assert!(parse_jin("999999999999999999999999999999999999").is_err());
    }

    #[test]
    fn format_boundaries() {
        assert_eq!(format_tong(0), "0铜");
        assert_eq!(format_tong(1), "1铜");
        assert_eq!(format_tong(99), "99铜");
        assert_eq!(format_tong(100), "1银");
        assert_eq!(format_tong(101), "1银1铜");
        assert_eq!(format_tong(10_000), "1金");
        assert_eq!(format_tong(12_003), "1金20银3铜");
        assert_eq!(format_tong(100_000_000), "1砖");
        assert_eq!(format_tong(100_020_003), "1砖2金3铜");
        assert_eq!(format_tong(-1), "负1铜");
        assert_eq!(format_tong(-12_003), "负1金20银3铜");
        // 极值只检查不 panic、正数可解析回
        assert!(format_tong(i64::MIN).starts_with('负'));
        assert_eq!(parse_jin(&format_tong(i64::MAX)).unwrap(), i64::MAX);
        assert_eq!(parse_jin(&format_tong(i64::MIN)).unwrap(), i64::MIN);
    }

    #[test]
    fn format_parse_roundtrip() {
        // 覆盖各进位/借位边界与负数
        let cases = [
            0_i64,
            1,
            2,
            99,
            100,
            101,
            9_999,
            10_000,
            10_001,
            12_345,
            99_999_999,
            100_000_000,
            100_000_001,
            123_456_789,
            9_876_543_210,
            -1,
            -99,
            -100,
            -12_003,
            -100_000_000,
            -123_456_789,
            i64::MAX,
            i64::MIN,
        ];
        for v in cases {
            let text = format_tong(v);
            assert_eq!(parse_jin(&text).unwrap(), v, "roundtrip {v} -> {text}");
        }
    }
}
