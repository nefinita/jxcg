//! 交易行费用：5% 手续费（默认四舍五入到铜）+ 保管费与保本价。
//!
//! 口径来源见 PLAN.md §6.2：wind-jx3 CostCalculator（feeRate=0.05、HALF_UP、custodyFee=8000）。

use crate::{JxcgError, Result};
use jxcg_api::{NetRequest, NetResult, Quzheng, Shouxufei, Tong};

/// 按成交额与费率算手续费（铜）。
pub fn jisuan_shouxufei(amount: Tong, cfg: &Shouxufei) -> Tong {
    if amount <= 0 || cfg.shuilv_bp <= 0 {
        return 0;
    }
    let raw = amount as i128 * cfg.shuilv_bp as i128;
    let fee = match cfg.quzheng {
        Quzheng::Floor => raw / 10_000,
        Quzheng::Ceil => (raw + 9_999) / 10_000,
        Quzheng::HalfUp => (raw + 5_000) / 10_000,
    };
    let fee = fee.max(0);
    if fee > amount as i128 { amount } else { fee as Tong }
}

/// 计算交易行净收入。
pub fn jisuan_net(req: &NetRequest) -> Result<NetResult> {
    if req.shoujia < 0 {
        return Err(JxcgError::Money("售价不能为负".into()));
    }
    if cfg_rate_invalid(&req.shouxufei) {
        return Err(JxcgError::Data("手续费率必须在 0%~100% 之间".into()));
    }
    let zonge = req
        .shoujia
        .checked_mul(req.shuliang as Tong)
        .ok_or_else(|| JxcgError::Data("金额溢出".into()))?;
    let feiyong = jisuan_shouxufei(zonge, &req.shouxufei);
    let baoguanfei = req
        .shouxufei
        .baoguanfei
        .checked_mul(req.guadan_cishu as Tong)
        .ok_or_else(|| JxcgError::Data("保管费溢出".into()))?;
    let daoshou = zonge
        .checked_sub(feiyong)
        .and_then(|v| v.checked_sub(baoguanfei))
        .ok_or_else(|| JxcgError::Data("金额溢出".into()))?;
    Ok(NetResult {
        zonge,
        feiyong,
        baoguanfei,
        daoshou,
    })
}

/// 求保本总售价：满足「售价 - 手续费 - 保管费 >= 成本」的最小售价（铜）。
pub fn baobenjia(chengben: Tong, cfg: &Shouxufei, guadan_cishu: u32) -> Result<Tong> {
    if chengben < 0 {
        return Err(JxcgError::Money("成本不能为负".into()));
    }
    if cfg_rate_invalid(cfg) {
        return Err(JxcgError::Data("手续费率必须在 0%~100% 之间".into()));
    }
    let custody = cfg
        .baoguanfei
        .checked_mul(guadan_cishu as Tong)
        .ok_or_else(|| JxcgError::Data("保管费溢出".into()))?;
    let target = chengben
        .checked_add(custody)
        .ok_or_else(|| JxcgError::Data("金额溢出".into()))?;
    if target == 0 {
        return Ok(0);
    }
    // 指数扩张找上界，再二分。
    let mut hi: Tong = 1;
    loop {
        if hi - jisuan_shouxufei(hi, cfg) >= target {
            break;
        }
        if hi > i64::MAX / 2 {
            return Err(JxcgError::Data("保本价超出范围".into()));
        }
        hi *= 2;
    }
    let mut lo: Tong = 0;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if mid - jisuan_shouxufei(mid, cfg) >= target {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    Ok(lo)
}

fn cfg_rate_invalid(cfg: &Shouxufei) -> bool {
    cfg.shuilv_bp < 0 || cfg.shuilv_bp >= 10_000
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
    fn fee_default_5pct() {
        // 1200 金 = 12_000_000 铜 → 手续费 60 金
        assert_eq!(jisuan_shouxufei(12_000_000, &cfg(500, Quzheng::HalfUp)), 600_000);
        // 5% of 10 铜 = 0.5 铜，四舍五入到 1 铜
        assert_eq!(jisuan_shouxufei(10, &cfg(500, Quzheng::HalfUp)), 1);
        assert_eq!(jisuan_shouxufei(10, &cfg(500, Quzheng::Floor)), 0);
        assert_eq!(jisuan_shouxufei(10, &cfg(500, Quzheng::Ceil)), 1);
        // 金额为 0 / 费率为 0
        assert_eq!(jisuan_shouxufei(0, &cfg(500, Quzheng::HalfUp)), 0);
        assert_eq!(jisuan_shouxufei(100, &cfg(0, Quzheng::HalfUp)), 0);
    }

    #[test]
    fn net_result() {
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
        assert_eq!(r.daoshou, 11_392_000);
    }

    #[test]
    fn net_multi_and_negative() {
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
    }

    #[test]
    fn rate_invalid() {
        let mut req = NetRequest {
            shoujia: 100,
            shuliang: 1,
            shouxufei: cfg(10_000, Quzheng::HalfUp),
            guadan_cishu: 0,
        };
        assert!(jisuan_net(&req).is_err());
        req.shouxufei.shuilv_bp = 500;
        assert!(jisuan_net(&req).is_ok());
    }

    #[test]
    fn baoben_boundary() {
        let cfg = cfg(500, Quzheng::HalfUp);
        // 成本 1000 金 + 保管费 8000 铜
        let cost = 10_000_000;
        let p = baobenjia(cost, &cfg, 1).unwrap();
        let net = p - jisuan_shouxufei(p, &cfg) - cfg.baoguanfei;
        assert!(net >= cost, "保本价 {p} 的净收入 {net} 应 >= 成本 {cost}");
        let prev = p - 1;
        let net_prev = prev - jisuan_shouxufei(prev, &cfg) - cfg.baoguanfei;
        assert!(net_prev < cost, "保本价应是最小值：{prev} 的净收入 {net_prev} 应 < 成本 {cost}");
    }

    #[test]
    fn baoben_zero_cost() {
        let cfg = cfg(500, Quzheng::HalfUp);
        // 成本 0 且无保管费 → 0
        let mut c = cfg.clone();
        c.baoguanfei = 0;
        assert_eq!(baobenjia(0, &c, 0).unwrap(), 0);
        // 成本 0 但有保管费 → 至少要覆盖保管费
        let p = baobenjia(0, &cfg, 1).unwrap();
        assert!(p - jisuan_shouxufei(p, &cfg) >= cfg.baoguanfei);
    }
}
