//! 交易行费用：5% 手续费（默认四舍五入到铜）+ 保管费与保本价。
//!
//! 口径来源见 PLAN.md §6.2：wind-jx3 CostCalculator（feeRate=0.05、HALF_UP、custodyFee=8000）。
//! - 手续费按成交总额计算，默认 HALF_UP 到铜；
//! - 保管费按挂单次数计（默认 80 银 = 8000 铜 / 24h / 次）；
//! - 到手 = 成交总额 − 手续费 − 保管费（可为负，提示「不划算」）；
//! - 保本价 = 满足「到手 >= 成本」的**最小**整数售价（逐铜最小性由测试钉死）。

use crate::{JxcgError, Result};
use jxcg_api::{NetRequest, NetResult, Quzheng, Shouxufei, Tong};

/// 合法费率上界（万分比，不含）：10000 = 100% 时任何正成本都无解，直接拒绝。
pub const SHUILV_BP_MAX_EXCLUSIVE: i64 = 10_000;

/// 校验值集里的手续费配置。
///
/// - 费率：`0 <= shuilv_bp < 10000`（0% 合法；100% 及以上无解，拒绝）；
/// - 保管费：`>= 0`（负保管费没有现实含义）。
pub fn validate_shouxufei(cfg: &Shouxufei) -> Result<()> {
    if cfg.shuilv_bp < 0 || cfg.shuilv_bp >= SHUILV_BP_MAX_EXCLUSIVE {
        return Err(JxcgError::Data(format!(
            "手续费率必须在 0~9999 万分比之间（0%~100%，不含 100%），当前 {}",
            cfg.shuilv_bp
        )));
    }
    if cfg.baoguanfei < 0 {
        return Err(JxcgError::Data(format!(
            "保管费不能为负，当前 {} 铜",
            cfg.baoguanfei
        )));
    }
    Ok(())
}

/// 按成交额与费率算手续费（铜）。
///
/// 纯函数防御：金额 <= 0 或费率 <= 0 返回 0；费率非法地 >= 10000 时不会返回超过金额的手续费。
/// 业务入口（net/baoben/craft/convert）应先用 [`validate_shouxufei`] 校验。
pub fn jisuan_shouxufei(amount: Tong, cfg: &Shouxufei) -> Tong {
    if amount <= 0 || cfg.shuilv_bp <= 0 {
        return 0;
    }
    let raw = amount as i128 * cfg.shuilv_bp as i128;
    let fee = match cfg.quzheng {
        Quzheng::Floor => raw / 10_000,
        Quzheng::Ceil => (raw + 9_999) / 10_000,
        // HALF_UP：0.5 铜向上进位，社区口径
        Quzheng::HalfUp => (raw + 5_000) / 10_000,
    };
    // 手续费不可能超过成交额本身
    fee.clamp(0, amount as i128) as Tong
}

/// 价格扣手续费后的净额（不含保管费）。
fn jingshouru_after_fee(price: Tong, cfg: &Shouxufei) -> Tong {
    price - jisuan_shouxufei(price, cfg)
}

/// 计算交易行净收入（到手 = 成交总额 − 手续费 − 保管费）。
pub fn jisuan_net(req: &NetRequest) -> Result<NetResult> {
    if req.shoujia < 0 {
        return Err(JxcgError::Money("售价不能为负".into()));
    }
    validate_shouxufei(&req.shouxufei)?;
    let zonge = req
        .shoujia
        .checked_mul(req.shuliang as Tong)
        .ok_or_else(|| JxcgError::Data("成交总额溢出（售价 × 数量）".into()))?;
    let feiyong = jisuan_shouxufei(zonge, &req.shouxufei);
    let baoguanfei = req
        .shouxufei
        .baoguanfei
        .checked_mul(req.guadan_cishu as Tong)
        .ok_or_else(|| JxcgError::Data("保管费溢出（单次保管费 × 挂单次数）".into()))?;
    let daoshou = zonge
        .checked_sub(feiyong)
        .and_then(|v| v.checked_sub(baoguanfei))
        .ok_or_else(|| JxcgError::Data("到手金额溢出".into()))?;
    Ok(NetResult {
        zonge,
        feiyong,
        baoguanfei,
        daoshou,
    })
}

/// 求保本总售价：满足「售价 − 手续费 − 保管费 >= 成本」的最小整数售价（铜）。
///
/// 逐铜最小性：返回值 `p` 满足条件，且 `p - 1` 不满足（`p > 0` 时）；
/// 用「指数扩上界 + 二分」求最小解，避免浮点近似导致差 1 铜。
pub fn baobenjia(chengben: Tong, cfg: &Shouxufei, guadan_cishu: u32) -> Result<Tong> {
    if chengben < 0 {
        return Err(JxcgError::Money("成本不能为负".into()));
    }
    validate_shouxufei(cfg)?;
    let custody = cfg
        .baoguanfei
        .checked_mul(guadan_cishu as Tong)
        .ok_or_else(|| JxcgError::Data("保管费溢出（单次保管费 × 挂单次数）".into()))?;
    let target = chengben
        .checked_add(custody)
        .ok_or_else(|| JxcgError::Data("保本目标金额溢出".into()))?;
    if target == 0 {
        // 零成本且无保管费：0 铜即保本，也是最小非负售价
        return Ok(0);
    }

    // 指数扩张找可行上界：费率 < 100% 时目标一定可达。
    let mut hi: Tong = 1;
    loop {
        if jingshouru_after_fee(hi, cfg) >= target {
            break;
        }
        if hi > i64::MAX / 2 {
            return Err(JxcgError::Data("保本价超出可计算范围".into()));
        }
        hi *= 2;
    }

    // 二分找最小可行价（净收入随售价单调不减）。
    let mut lo: Tong = 0;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if jingshouru_after_fee(mid, cfg) >= target {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    Ok(lo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jxcg_api::{NetRequest, Quzheng};

    fn cfg(bp: i64, quzheng: Quzheng) -> Shouxufei {
        Shouxufei {
            shuilv_bp: bp,
            quzheng,
            baoguanfei: 8_000,
        }
    }

    #[test]
    fn fee_half_up_boundaries() {
        let half_up = cfg(500, Quzheng::HalfUp);
        let floor = cfg(500, Quzheng::Floor);
        let ceil = cfg(500, Quzheng::Ceil);
        // 5% 的手续费 = amount × 500 / 10000，按铜四舍五入
        let cases = [
            (0_i64, 0_i64, 0_i64, 0_i64),
            (1, 0, 0, 1),
            (9, 0, 0, 1),
            (10, 1, 0, 1), // 0.5 铜 → HALF_UP 进位
            (11, 1, 0, 1),
            (19, 1, 0, 1),
            (20, 1, 1, 1),
            (29, 1, 1, 2), // 1.45 铜
            (30, 2, 1, 2), // 1.5 铜 → 进位
            (100, 5, 5, 5),
            (1234, 62, 61, 62), // 61.7 铜
        ];
        for (amount, expect_half_up, expect_floor, expect_ceil) in cases {
            assert_eq!(
                jisuan_shouxufei(amount, &half_up),
                expect_half_up,
                "HALF_UP amount={amount}"
            );
            assert_eq!(
                jisuan_shouxufei(amount, &floor),
                expect_floor,
                "FLOOR amount={amount}"
            );
            assert_eq!(
                jisuan_shouxufei(amount, &ceil),
                expect_ceil,
                "CEIL amount={amount}"
            );
        }
        // 0% 费率
        assert_eq!(jisuan_shouxufei(12_000_000, &cfg(0, Quzheng::HalfUp)), 0);
        // 极端费率 9999 万分比时手续费不超过成交额
        let high = cfg(9_999, Quzheng::HalfUp);
        assert_eq!(jisuan_shouxufei(10_000, &high), 9_999);
        assert!(jisuan_shouxufei(1, &high) <= 1);
        // 负数成交额（防御）不产生负手续费
        assert_eq!(jisuan_shouxufei(-100, &half_up), 0);
    }

    #[test]
    fn validate_config() {
        assert!(validate_shouxufei(&cfg(0, Quzheng::HalfUp)).is_ok());
        assert!(validate_shouxufei(&cfg(500, Quzheng::HalfUp)).is_ok());
        assert!(validate_shouxufei(&cfg(9_999, Quzheng::HalfUp)).is_ok());
        assert!(validate_shouxufei(&cfg(-1, Quzheng::HalfUp)).is_err());
        assert!(validate_shouxufei(&cfg(10_000, Quzheng::HalfUp)).is_err());
        assert!(validate_shouxufei(&cfg(20_000, Quzheng::HalfUp)).is_err());
        let mut negative_custody = cfg(500, Quzheng::HalfUp);
        negative_custody.baoguanfei = -1;
        assert!(validate_shouxufei(&negative_custody).is_err());
    }

    #[test]
    fn net_plan_example() {
        // PLAN.md §6.4：售价 1200 金，5% = 60 金（HALF_UP），保管费 80 银
        let req = NetRequest {
            shoujia: 12_000_000,
            shuliang: 1,
            shouxufei: cfg(500, Quzheng::HalfUp),
            guadan_cishu: 1,
        };
        let r = jisuan_net(&req).unwrap();
        assert_eq!(r.zonge, 12_000_000);
        assert_eq!(r.feiyong, 600_000);
        assert_eq!(r.baoguanfei, 8_000);
        assert_eq!(r.daoshou, 11_392_000); // 1139 金 20 银
    }

    #[test]
    fn net_batch_and_negative() {
        let req = NetRequest {
            shoujia: 100,
            shuliang: 3,
            shouxufei: cfg(500, Quzheng::HalfUp),
            guadan_cishu: 1,
        };
        let r = jisuan_net(&req).unwrap();
        assert_eq!(r.zonge, 300);
        assert_eq!(r.feiyong, 15); // 5% of 300
        assert_eq!(r.daoshou, 300 - 15 - 8_000);
        // 保管费大于销售额 → 到手为负，是合法结果（提示用户不划算）
        assert!(r.daoshou < 0);
        // 零售价也要付保管费
        let zero = NetRequest {
            shoujia: 0,
            shuliang: 1,
            shouxufei: cfg(500, Quzheng::HalfUp),
            guadan_cishu: 1,
        };
        let r = jisuan_net(&zero).unwrap();
        assert_eq!(r.zonge, 0);
        assert_eq!(r.daoshou, -8_000);
        // 多挂单次数按次累计保管费
        let multi = NetRequest {
            shoujia: 12_000_000,
            shuliang: 1,
            shouxufei: cfg(500, Quzheng::HalfUp),
            guadan_cishu: 2,
        };
        let r = jisuan_net(&multi).unwrap();
        assert_eq!(r.baoguanfei, 16_000);
        assert_eq!(r.daoshou, 12_000_000 - 600_000 - 16_000);
    }

    #[test]
    fn net_errors() {
        let mut req = NetRequest {
            shoujia: 100,
            shuliang: 1,
            shouxufei: cfg(10_000, Quzheng::HalfUp),
            guadan_cishu: 0,
        };
        assert!(jisuan_net(&req).is_err());
        req.shouxufei.shuilv_bp = 500;
        assert!(jisuan_net(&req).is_ok());
        req.shoujia = -1;
        assert!(jisuan_net(&req).is_err());
        // 售价 × 数量溢出
        let overflow = NetRequest {
            shoujia: i64::MAX,
            shuliang: 2,
            shouxufei: cfg(500, Quzheng::HalfUp),
            guadan_cishu: 0,
        };
        assert!(jisuan_net(&overflow).is_err());
        // 阴性保管费由 validate 拦截
        let mut bad_custody = cfg(500, Quzheng::HalfUp);
        bad_custody.baoguanfei = -1;
        let req = NetRequest {
            shoujia: 100,
            shuliang: 1,
            shouxufei: bad_custody,
            guadan_cishu: 1,
        };
        assert!(jisuan_net(&req).is_err());
    }

    #[test]
    fn baoben_minimality() {
        // 多组费率 × 成本，验证「p 可行且 p-1 不可行」
        for bp in [0_i64, 1, 500, 4_878, 9_999] {
            let cfg = cfg(bp, Quzheng::HalfUp);
            for cost in [0_i64, 1, 99, 8_000, 10_000, 12_345_678, 1_000_000_000] {
                for cishu in [0_u32, 1, 3] {
                    let p = baobenjia(cost, &cfg, cishu).unwrap();
                    let custody = cfg.baoguanfei * cishu as i64;
                    let net = p - jisuan_shouxufei(p, &cfg) - custody;
                    assert!(
                        net >= cost,
                        "保本价 {p}（bp={bp}, cost={cost}, cishu={cishu}）净收入 {net} 应 >= 成本"
                    );
                    if p > 0 {
                        let prev = p - 1;
                        let net_prev = prev - jisuan_shouxufei(prev, &cfg) - custody;
                        assert!(
                            net_prev < cost,
                            "保本价应最小：{prev}（bp={bp}, cost={cost}）净收入 {net_prev} 应 < {cost}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn baoben_zero_cost() {
        let cfg500 = cfg(500, Quzheng::HalfUp);
        // 成本 0 且无保管费 → 0
        let mut no_custody = cfg500.clone();
        no_custody.baoguanfei = 0;
        assert_eq!(baobenjia(0, &no_custody, 0).unwrap(), 0);
        // 成本 0 但有保管费 → 至少覆盖保管费
        let p = baobenjia(0, &cfg500, 1).unwrap();
        assert!(p - jisuan_shouxufei(p, &cfg500) >= 8_000);
        // 0% 费率时保本价 = 成本 + 保管费
        let zero_rate = cfg(0, Quzheng::HalfUp);
        assert_eq!(
            baobenjia(1_234, &cfg(0, Quzheng::HalfUp), 2).unwrap(),
            1_234 + 16_000
        );
        assert!(baobenjia(1, &zero_rate, 0).is_ok());
    }

    #[test]
    fn baoben_errors() {
        let cfg500 = cfg(500, Quzheng::HalfUp);
        assert!(baobenjia(-1, &cfg500, 0).is_err());
        assert!(baobenjia(100, &cfg(10_000, Quzheng::HalfUp), 0).is_err());
        assert!(baobenjia(i64::MAX, &cfg500, 1).is_err());
        let mut bad = cfg500.clone();
        bad.baoguanfei = -1;
        assert!(baobenjia(100, &bad, 1).is_err());
    }

    #[test]
    fn baoben_rounding_edges() {
        // 构造手续费取整跳变：成本 10 铜，保管费 0，5% 时保本价应为 11
        let mut cfg = cfg(500, Quzheng::HalfUp);
        cfg.baoguanfei = 0;
        let p = baobenjia(10, &cfg, 0).unwrap();
        let prev = p - 1;
        assert!(p - jisuan_shouxufei(p, &cfg) >= 10);
        assert!(prev - jisuan_shouxufei(prev, &cfg) < 10);
    }
}
