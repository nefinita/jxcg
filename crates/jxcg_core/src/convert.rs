//! 五行石精炼（转换配方）计算。
//!
//! 模型（PLAN.md §6.3）：每次投入 `xuyao` 个低阶石 + 精力 + 金币，成功率 `p`；
//! 由几何分布期望，**产出 1 个高阶石平均需要 1/p 次投入**。
//!
//! 口径（与 Lead 确认，2026-11-12）：
//! - `cishu` = **目标成功次数**（产出个数）；
//! - 期望投入次数 `qiwang_cishu` = cishu × 10000 / chenggonglv_bp（即 cishu / p，合计值）；
//! - 期望成本 = 单次投入成本 × cishu/p，最后一次性 HALF_UP 到铜；
//! - 期望收入（扣手续费前）= cishu × 高阶石单价；
//! - 成功率 `chenggonglv_bp` clamp 到 1..10000（0 或负数据按 0.01% 处理，>100% 按 100% 处理）；
//! - 单价优先级与 craft 一致：行情/手填 > 商店价 > 缺价（缺价按 0 计并写入 `quejia`）。

use std::collections::BTreeMap;

use crate::{JxcgError, Result, fee, math};
use jxcg_api::{Catalog, Tong, ZhuanhuanRequest, ZhuanhuanResult};

/// 成功率万分比下限/上限：clamp 到 1..=10000（0.01% ~ 100%）。
pub const CHENGGONGLV_BP_MIN: i64 = 1;
pub const CHENGGONGLV_BP_MAX: i64 = 10_000;

/// 成功率（万分比），clamp 到合法区间。
pub fn clamp_chenggonglv_bp(bp: i64) -> i64 {
    bp.clamp(CHENGGONGLV_BP_MIN, CHENGGONGLV_BP_MAX)
}

/// 取单价：行情/手填 > 商店价 > 缺价（与 craft 口径一致）。
fn qu_danjia(
    catalog: &Catalog,
    jiage: &BTreeMap<String, Tong>,
    wupin_id: &str,
    quejia: &mut Vec<String>,
) -> Result<(Tong, &'static str)> {
    if let Some(price) = jiage.get(wupin_id) {
        if *price < 0 {
            return Err(JxcgError::Money(format!(
                "行情/手填单价不能为负：{wupin_id} = {price}"
            )));
        }
        return Ok((*price, "hangqing"));
    }
    if let Some(wupin) = catalog.wupin.iter().find(|w| w.id == wupin_id)
        && let Some(price) = wupin.shangdianjia
    {
        if price < 0 {
            return Err(JxcgError::Data(format!(
                "商店价不能为负：{wupin_id} = {price}"
            )));
        }
        return Ok((price, "shangdian"));
    }
    quejia.push(wupin_id.to_string());
    Ok((0, "quejia"))
}

/// 计算五行石转换的期望成本/收入/利润。
pub fn jisuan(catalog: &Catalog, req: &ZhuanhuanRequest) -> Result<ZhuanhuanResult> {
    if req.cishu == 0 {
        return Err(JxcgError::Data("转换次数必须大于 0".into()));
    }
    if req.jingli_danjia < 0 {
        return Err(JxcgError::Money("精力单价不能为负".into()));
    }
    fee::validate_shouxufei(&req.shouxufei)?;

    let z = catalog
        .wuxingshi
        .iter()
        .find(|z| z.id == req.zhuanhuan_id)
        .ok_or_else(|| JxcgError::Data(format!("转换配方不存在：{}", req.zhuanhuan_id)))?;
    if z.jinbi < 0 {
        return Err(JxcgError::Data(format!(
            "转换金币消耗不能为负：{} = {}",
            z.id, z.jinbi
        )));
    }
    let cishu = req.cishu as i128;
    let mut quejia: Vec<String> = Vec::new();

    let (congjia, _cong_laiyuan) = qu_danjia(catalog, &req.jiage, &z.cong, &mut quejia)?;
    let (daojia, _dao_laiyuan) = qu_danjia(catalog, &req.jiage, &z.dao, &mut quejia)?;

    // 成功率 clamp 到 1..=10000，避免 0 除、负数与 >100% 的荒谬结果。
    let chenggonglv_bp = clamp_chenggonglv_bp(z.chenggonglv_bp) as i128;
    let qiwang_cishu_per_success = 10_000.0 / chenggonglv_bp as f64;

    // 单次投入成本（低阶石 + 金币 + 精力）
    let yici_chengben = math::checked_add(
        math::checked_add(
            math::checked_mul(congjia as i128, z.xuyao as i128, "低阶石投入")?,
            z.jinbi as i128,
            "单次投入成本",
        )?,
        math::checked_mul(z.jingli as i128, req.jingli_danjia as i128, "精力成本")?,
        "单次投入成本",
    )?;

    // 期望投入次数 = cishu × 10000 / bp（分子先放大，最后一次性取整）
    let qiwang_num = math::checked_mul(cishu, 10_000, "期望投入次数")?;
    let zong_chengben_i = math::checked_mul(yici_chengben, qiwang_num, "期望总成本")?;
    let zong_chengben = math::to_tong(
        math::round_div_half_up(zong_chengben_i, chenggonglv_bp),
        "期望总成本",
    )?;

    // 期望收入（扣手续费前）= 目标成功次数 × 高阶石单价
    let shouru_i = math::checked_mul(cishu, daojia as i128, "期望收入")?;
    let shouru = math::to_tong(shouru_i, "期望收入")?;
    let feiyong = fee::jisuan_shouxufei(shouru, &req.shouxufei);
    let jingshouru = shouru - feiyong;
    let lirun = jingshouru
        .checked_sub(zong_chengben)
        .ok_or_else(|| JxcgError::Data("净利润溢出".into()))?;
    let lirunlv_bp = if zong_chengben > 0 {
        math::to_tong(
            math::round_div_half_up_signed(lirun as i128 * 10_000, zong_chengben as i128),
            "利润率",
        )?
    } else {
        0
    };

    quejia.sort();
    quejia.dedup();

    Ok(ZhuanhuanResult {
        zhuanhuan_id: z.id.clone(),
        mingcheng: z.mingcheng.clone(),
        qiwang_cishu: qiwang_cishu_per_success * req.cishu as f64,
        zong_chengben,
        shouru,
        lirun,
        lirunlv_bp,
        quejia,
    })
}

/// 便捷函数：直接用 id + 价格表计算。
pub fn jisuan_by_price(
    catalog: &Catalog,
    zhuanhuan_id: &str,
    jiage: &BTreeMap<String, Tong>,
    req: &ZhuanhuanRequest,
) -> Result<ZhuanhuanResult> {
    let mut next = req.clone();
    next.zhuanhuan_id = zhuanhuan_id.to_string();
    next.jiage = jiage.clone();
    jisuan(catalog, &next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jxcg_api::{Banben, Pinlei, Quzheng, Shouxufei, Wupin, Zhuanhuan};

    fn wupin(id: &str, shop: Option<Tong>) -> Wupin {
        Wupin {
            id: id.into(),
            mingcheng: format!("物品{id}"),
            pinlei: Pinlei::Wuxingshi,
            dui: None,
            shangdianjia: shop,
            laiyuan: None,
        }
    }

    fn zhuanhuan(id: &str, bp: i64) -> Zhuanhuan {
        Zhuanhuan {
            id: id.into(),
            mingcheng: format!("转换{id}"),
            cong: "stone_low".into(),
            dao: "stone_high".into(),
            xuyao: 10,
            jingli: 5,
            jinbi: 100,
            chenggonglv_bp: bp,
            laiyuan: Some("test".into()),
        }
    }

    fn catalog(list: Vec<Zhuanhuan>) -> Catalog {
        Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("stone_low", None),
                wupin("stone_high", None),
                wupin("shop_low", Some(7)),
                wupin("shop_high", Some(100)),
            ],
            peifang: vec![],
            wuxingshi: list,
            fuwuqi: vec![],
            zhiji: vec![],
        }
    }

    fn request(id: &str) -> ZhuanhuanRequest {
        ZhuanhuanRequest {
            zhuanhuan_id: id.into(),
            cishu: 1,
            jingli_danjia: 0,
            shouxufei: Shouxufei {
                shuilv_bp: 500,
                quzheng: Quzheng::HalfUp,
                baoguanfei: 0,
            },
            jiage: BTreeMap::new(),
        }
    }

    #[test]
    fn qiwang_cost_one_over_p() {
        // 单次成本 = 低阶石 10×10 + 金币 100 + 精力 0 = 200；p=50% → 期望成本 400
        let cat = catalog(vec![zhuanhuan("t1", 5_000)]);
        let mut req = request("t1");
        req.jiage = BTreeMap::from([
            ("stone_low".to_string(), 10),
            ("stone_high".to_string(), 600),
        ]);
        let r = jisuan(&cat, &req).unwrap();
        assert!((r.qiwang_cishu - 2.0).abs() < 1e-9);
        assert_eq!(r.zong_chengben, 400);
        // 期望收入 = 1 × 600；手续费 30；净收入 570；利润 170
        assert_eq!(r.shouru, 600);
        assert_eq!(r.lirun, 170);
        assert_eq!(r.lirunlv_bp, 4_250); // 170/400 = 42.5%
        assert!(r.quejia.is_empty());
    }

    #[test]
    fn cishu_three() {
        let cat = catalog(vec![zhuanhuan("t1", 5_000)]);
        let mut req = request("t1");
        req.cishu = 3;
        req.jiage = BTreeMap::from([
            ("stone_low".to_string(), 10),
            ("stone_high".to_string(), 600),
        ]);
        let r = jisuan(&cat, &req).unwrap();
        // 期望投入 = 3 × 2 = 6 次，成本 6×200 = 1200；收入 3×600 = 1800；手续费 90
        assert!((r.qiwang_cishu - 6.0).abs() < 1e-9);
        assert_eq!(r.zong_chengben, 1_200);
        assert_eq!(r.shouru, 1_800);
        assert_eq!(r.lirun, 1_800 - 90 - 1_200);
    }

    #[test]
    fn chenggonglv_clamp() {
        let make = |bp: i64| {
            let cat = catalog(vec![zhuanhuan("t1", bp)]);
            let mut req = request("t1");
            req.jiage =
                BTreeMap::from([("stone_low".to_string(), 0), ("stone_high".to_string(), 0)]);
            jisuan(&cat, &req).unwrap()
        };
        // 0% → clamp 到 1bp（0.01%），期望投入 10000 次，不会除零/NaN
        let r = make(0);
        assert!((r.qiwang_cishu - 10_000.0).abs() < 1e-6);
        assert_eq!(r.zong_chengben, 1_000_000); // 100 × 10000
        let r = make(-5);
        assert!((r.qiwang_cishu - 10_000.0).abs() < 1e-6);
        // 100% → 期望投入 1 次
        let r = make(10_000);
        assert!((r.qiwang_cishu - 1.0).abs() < 1e-9);
        assert_eq!(r.zong_chengben, 100);
        // >100% → clamp 到 100%
        let r = make(20_000);
        assert!((r.qiwang_cishu - 1.0).abs() < 1e-9);
        assert_eq!(r.zong_chengben, 100);
        // 49% 附近：100 / 0.49 = 204.08 → 204 铜
        let cat = catalog(vec![zhuanhuan("t1", 4_900)]);
        let mut req = request("t1");
        req.jiage = BTreeMap::from([("stone_low".to_string(), 0), ("stone_high".to_string(), 0)]);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.zong_chengben, 204);
    }

    #[test]
    fn missing_prices() {
        let cat = catalog(vec![zhuanhuan("t1", 5_000)]);
        let req = request("t1");
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(
            r.quejia,
            vec!["stone_high".to_string(), "stone_low".to_string()]
        );
        // 缺价按 0 计：成本只剩金币 100 × 2 次 = 200；收入 0
        assert_eq!(r.zong_chengben, 200);
        assert_eq!(r.shouru, 0);
        assert_eq!(r.lirun, -200);
    }

    #[test]
    fn shop_price_fallback() {
        let cat = catalog(vec![zhuanhuan("t1", 10_000)]);
        // 转换配方引用 shop_low/shop_high（有商店价）
        let mut z = zhuanhuan("t2", 10_000);
        z.cong = "shop_low".into();
        z.dao = "shop_high".into();
        z.xuyao = 2;
        z.jinbi = 0;
        z.jingli = 0;
        let mut cat = cat;
        cat.wuxingshi.push(z);
        let mut req = request("t2");
        req.jiage = BTreeMap::new();
        let r = jisuan(&cat, &req).unwrap();
        assert!(r.quejia.is_empty());
        // 成本 = 2 × 7 = 14；收入 = 100；手续费 5 → 利润 81
        assert_eq!(r.zong_chengben, 14);
        assert_eq!(r.shouru, 100);
        assert_eq!(r.lirun, 81);
    }

    #[test]
    fn rounding_expected_cost() {
        // 单次成本 100、p=30% → 333.33 → 333 铜
        let cat = catalog(vec![zhuanhuan("t1", 3_000)]);
        let req = request("t1");
        assert_eq!(jisuan(&cat, &req).unwrap().zong_chengben, 333);
        // 单次成本 2、p=30% → 6.66 → 7 铜
        let mut z = zhuanhuan("t3", 3_000);
        z.xuyao = 0;
        z.jingli = 0;
        z.jinbi = 2;
        let cat3 = catalog(vec![z]);
        let mut req3 = request("t3");
        req3.jiage = BTreeMap::from([("stone_low".to_string(), 0), ("stone_high".to_string(), 0)]);
        assert_eq!(jisuan(&cat3, &req3).unwrap().zong_chengben, 7);
    }

    #[test]
    fn zero_cost() {
        // 全部免费：成本 0、收入 0，利润 0；利润率记 0
        let mut z = zhuanhuan("free", 5_000);
        z.xuyao = 0;
        z.jingli = 0;
        z.jinbi = 0;
        let cat = catalog(vec![z]);
        let mut req = request("free");
        req.jiage = BTreeMap::from([("stone_low".to_string(), 0), ("stone_high".to_string(), 0)]);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.zong_chengben, 0);
        assert_eq!(r.lirun, 0);
        assert_eq!(r.lirunlv_bp, 0);
    }

    #[test]
    fn errors() {
        let cat = catalog(vec![zhuanhuan("t1", 5_000)]);
        let mut req = request("t1");
        req.cishu = 0;
        assert!(jisuan(&cat, &req).is_err());
        let mut req = request("t1");
        req.jingli_danjia = -1;
        assert!(jisuan(&cat, &req).is_err());
        let mut req = request("t1");
        req.shouxufei.shuilv_bp = 10_000;
        assert!(jisuan(&cat, &req).is_err());
        let mut req = request("t1");
        req.jiage = BTreeMap::from([("stone_low".to_string(), -1)]);
        assert!(jisuan(&cat, &req).is_err());
        let req = request("nope");
        assert!(jisuan(&cat, &req).is_err());
        // 金币消耗为负的脏数据
        let mut bad = zhuanhuan("bad", 5_000);
        bad.jinbi = -1;
        let bad_cat = catalog(vec![bad]);
        assert!(jisuan(&bad_cat, &request("bad")).is_err());
    }

    #[test]
    fn by_price_helper() {
        let cat = catalog(vec![zhuanhuan("t1", 5_000)]);
        let req = request("t1");
        let jiage = BTreeMap::from([
            ("stone_low".to_string(), 10),
            ("stone_high".to_string(), 600),
        ]);
        let r = jisuan_by_price(&cat, "t1", &jiage, &req).unwrap();
        assert_eq!(r.zong_chengben, 400);
        assert_eq!(r.shouru, 600);
    }
}
