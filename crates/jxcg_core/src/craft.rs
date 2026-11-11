//! 单配方成本/利润计算。
//!
//! 口径（PLAN.md §6.3）：
//! - 材料成本 = Σ(需求数量 × 单价)，单价优先行情、其次商店价、否则记缺价；
//! - 期望产出 = Σ(权重/2^20 × (min+max)/2)；
//! - 抛光：手续费按产出总值扣 5%（HALF_UP），保管费按挂单次数计；
//! - 净利润 = 税后收入 − 材料 − 精力 − 制作费 − 保管费。

use std::collections::BTreeMap;

use crate::{JxcgError, Result, fee};
use jxcg_api::{
    CailiaoMingxi, Catalog, Chuchan, ChuchanMingxi, JisuanRequest, JisuanResult, QUANZHONG_FENMU,
    Tong, Wupin,
};

/// 单次制作的期望产出件数。
pub fn chuchan_qiwang(chuchan: &[Chuchan]) -> f64 {
    chuchan
        .iter()
        .map(|slot| {
            let p = if slot.quanzhong == 0 {
                0.0
            } else {
                slot.quanzhong as f64 / QUANZHONG_FENMU as f64
            };
            p * ((slot.min as f64 + slot.max as f64) / 2.0)
        })
        .sum()
}

fn find_wupin<'a>(catalog: &'a Catalog, id: &str) -> Option<&'a Wupin> {
    catalog.wupin.iter().find(|w| w.id == id)
}

fn chuchan_danjia(
    catalog: &Catalog,
    jiage: &BTreeMap<String, Tong>,
    wupin_id: &str,
    quejia: &mut Vec<String>,
) -> (Tong, String) {
    if let Some(price) = jiage.get(wupin_id) {
        return (*price, "hangqing".to_string());
    }
    if let Some(wupin) = find_wupin(catalog, wupin_id) {
        if let Some(price) = wupin.shangdianjia {
            return (price, "shangdian".to_string());
        }
    }
    quejia.push(wupin_id.to_string());
    (0, "quejia".to_string())
}

/// 计算一个配方的成本与利润。
pub fn jisuan(catalog: &Catalog, req: &JisuanRequest) -> Result<JisuanResult> {
    let peifang = catalog
        .peifang
        .iter()
        .find(|p| p.id == req.peifang_id)
        .ok_or_else(|| JxcgError::Data(format!("配方不存在：{}", req.peifang_id)))?;
    let cishu = req.cishu.max(1) as i64;
    let mut quejia: Vec<String> = Vec::new();

    // ---- 材料 ----
    let mut cailiao = Vec::with_capacity(peifang.cailiao.len());
    let mut chengben_cailiao: Tong = 0;
    for item in &peifang.cailiao {
        let need = item.shuliang as Tong * cishu;
        let (danjia, laiyuan) = chuchan_danjia(catalog, &req.jiage, &item.wupin_id, &mut quejia);
        let xiaojia = danjia.saturating_mul(need);
        chengben_cailiao = chengben_cailiao.saturating_add(xiaojia);
        cailiao.push(CailiaoMingxi {
            wupin_id: item.wupin_id.clone(),
            mingcheng: find_wupin(catalog, &item.wupin_id)
                .map(|w| w.mingcheng.clone())
                .unwrap_or_else(|| item.wupin_id.clone()),
            shuliang: item.shuliang,
            danjia,
            xiaojia,
            laiyuan,
        });
    }

    // ---- 精力 / 制作费 / 保管费 ----
    let jingli = peifang.jingli.saturating_mul(cishu as u32);
    let chengben_jingli = (jingli as Tong).saturating_mul(req.jingli_danjia);
    let zhizuofei = peifang.zhizuofei.saturating_mul(cishu);
    // 一次制作按一次挂单估算保管费（多档挂单时长后续在设置里展开）。
    let baoguanfei = req
        .shouxufei
        .baoguanfei
        .saturating_mul(req.guadan_cishu.max(1) as Tong * cishu);
    let zong_chengben = chengben_cailiao
        .saturating_add(chengben_jingli)
        .saturating_add(zhizuofei)
        .saturating_add(baoguanfei);

    // ---- 产出与收入 ----
    let qiwang_yici = chuchan_qiwang(&peifang.chuchan);
    let chuchan_qiwang_total = qiwang_yici * cishu as f64;
    let mut chuchan_mx = Vec::with_capacity(peifang.chuchan.len());
    let mut shouru: Tong = 0;
    for slot in &peifang.chuchan {
        let p = if slot.quanzhong == 0 {
            0.0
        } else {
            slot.quanzhong as f64 / QUANZHONG_FENMU as f64
        };
        let qiwang = p * ((slot.min as f64 + slot.max as f64) / 2.0) * cishu as f64;
        let (danjia, _laiyuan) = chuchan_danjia(catalog, &req.jiage, &slot.wupin_id, &mut quejia);
        shouru = shouru.saturating_add((qiwang * danjia as f64).round() as Tong);
        chuchan_mx.push(ChuchanMingxi {
            wupin_id: slot.wupin_id.clone(),
            mingcheng: find_wupin(catalog, &slot.wupin_id)
                .map(|w| w.mingcheng.clone())
                .unwrap_or_else(|| slot.wupin_id.clone()),
            qiwang,
            danjia,
        });
    }
    if !(0..10_000).contains(&req.shouxufei.shuilv_bp) {
        return Err(JxcgError::Data("手续费率必须在 0%~100% 之间".into()));
    }
    let feiyong = fee::jisuan_shouxufei(shouru, &req.shouxufei);
    let jingshouru = shouru.saturating_sub(feiyong);

    // ---- 利润指标 ----
    let lirun = jingshouru.saturating_sub(zong_chengben);
    let lirunlv_bp = if zong_chengben > 0 {
        (lirun as f64 / zong_chengben as f64 * 10_000.0).round() as i64
    } else {
        0
    };
    let mei_jingli = if jingli > 0 {
        lirun / jingli as Tong
    } else {
        lirun
    };
    // 保本总售价（成本已含保管费，这里不再重复计挂单次数）。
    let baoben_total = fee::baobenjia(zong_chengben, &req.shouxufei, 0)?;
    let baobenjia = if chuchan_qiwang_total > 0.0 {
        (baoben_total as f64 / chuchan_qiwang_total).ceil() as Tong
    } else {
        baoben_total
    };

    // 去重缺价列表
    quejia.sort();
    quejia.dedup();

    Ok(JisuanResult {
        peifang_id: peifang.id.clone(),
        peifang_mingcheng: peifang.mingcheng.clone(),
        chengben_cailiao,
        chengben_jingli,
        zhizuofei,
        baoguanfei,
        zong_chengben,
        shouru,
        jingshouru,
        lirun,
        lirunlv_bp,
        mei_jingli,
        baobenjia,
        jingli,
        chuchan_qiwang: chuchan_qiwang_total,
        cailiao,
        chuchan: chuchan_mx,
        quejia,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jxcg_api::{Banben, Cailiao, Chuchan, Jiyi, Peifang, Quzheng, Shouxufei, Wupin};

    fn catalog() -> Catalog {
        Catalog {
            banben: Banben::default(),
            wupin: vec![
                Wupin {
                    id: "5_1".into(),
                    mingcheng: "材料".into(),
                    pinlei: jxcg_api::Pinlei::Cailiao,
                    dui: Some(20),
                    shangdianjia: Some(100),
                    laiyuan: None,
                },
                Wupin {
                    id: "5_2".into(),
                    mingcheng: "成品".into(),
                    pinlei: jxcg_api::Pinlei::Chengpin,
                    dui: None,
                    shangdianjia: None,
                    laiyuan: None,
                },
            ],
            peifang: vec![Peifang {
                id: "cooking_1".into(),
                mingcheng: "测试配方".into(),
                jiyi: Jiyi::Pengren,
                dengji: 10,
                jingli: 20,
                zhizuofei: 0,
                cailiao: vec![Cailiao {
                    wupin_id: "5_1".into(),
                    shuliang: 2,
                }],
                chuchan: vec![Chuchan {
                    wupin_id: "5_2".into(),
                    min: 1,
                    max: 1,
                    quanzhong: QUANZHONG_FENMU,
                }],
                laiyuan: None,
            }],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        }
    }

    fn shouxufei() -> Shouxufei {
        Shouxufei {
            shuilv_bp: 500,
            quzheng: Quzheng::HalfUp,
            baoguanfei: 0,
        }
    }

    #[test]
    fn qiwang_weighted() {
        let slots = vec![
            Chuchan {
                wupin_id: "a".into(),
                min: 2,
                max: 3,
                quanzhong: 1_006_633,
            },
            Chuchan {
                wupin_id: "b".into(),
                min: 2,
                max: 3,
                quanzhong: 41_943,
            },
        ];
        // 两个槽权重加起来正好 2^20，期望 = 2.5
        assert!((chuchan_qiwang(&slots) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn jisuan_basic() {
        let catalog = catalog();
        let req = JisuanRequest {
            peifang_id: "cooking_1".into(),
            cishu: 1,
            guadan_cishu: 1,
            jingli_danjia: 10,
            shouxufei: shouxufei(),
            jiage: BTreeMap::from([
                ("5_1".to_string(), 200),
                ("5_2".to_string(), 1_000),
            ]),
            jiyi_dengji: 0,
        };
        let r = jisuan(&catalog, &req).unwrap();
        // 材料 2×200 = 400；精力 20×10 = 200；总成本 600
        assert_eq!(r.chengben_cailiao, 400);
        assert_eq!(r.chengben_jingli, 200);
        assert_eq!(r.zong_chengben, 600);
        // 收入 1000，手续费 50，净收入 950；利润 350
        assert_eq!(r.shouru, 1_000);
        assert_eq!(r.jingshouru, 950);
        assert_eq!(r.lirun, 350);
        assert!(r.quejia.is_empty());
    }

    #[test]
    fn jisuan_missing_price() {
        let catalog = catalog();
        let req = JisuanRequest {
            peifang_id: "cooking_1".into(),
            cishu: 1,
            guadan_cishu: 1,
            jingli_danjia: 0,
            shouxufei: shouxufei(),
            jiage: BTreeMap::new(), // 材料走商店价 100，产出缺价
            jiyi_dengji: 0,
        };
        let r = jisuan(&catalog, &req).unwrap();
        assert_eq!(r.chengben_cailiao, 200); // 2 × 商店价 100
        assert_eq!(r.quejia, vec!["5_2".to_string()]);
    }

    #[test]
    fn jisuan_unknown_recipe() {
        let catalog = catalog();
        let req = JisuanRequest {
            peifang_id: "nope".into(),
            cishu: 1,
            guadan_cishu: 1,
            jingli_danjia: 0,
            shouxufei: shouxufei(),
            jiage: BTreeMap::new(),
            jiyi_dengji: 0,
        };
        assert!(jisuan(&catalog, &req).is_err());
    }
}
