//! 金标准集成测试：从 crate 外部走公开 API，钉死 PLAN.md §6 的关键口径。
//!
//! 与各模块单测互补：这里覆盖「UI 实际调用路径」的端到端数字，
//! 数据用测试内合成目录（不依赖 data/ 随包数据，避免数据导出后测试漂移）。

use std::collections::{BTreeMap, BTreeSet};

use jxcg_api::{
    Banben, Cailiao, Catalog, Chuchan, JisuanRequest, Jiyi, NetRequest, PaihangRequest,
    PaihangZhibiao, Peifang, Pinlei, QUANZHONG_FENMU, Quzheng, Shouxufei, Wupin, Zhuanhuan,
    ZhuanhuanRequest,
};
use jxcg_core::{convert, craft, fee, money, rank};

fn cfg() -> Shouxufei {
    Shouxufei {
        shuilv_bp: 500,
        quzheng: Quzheng::HalfUp,
        baoguanfei: 8_000, // 80 银 / 24h
    }
}

fn wupin(id: &str, mingcheng: &str, pinlei: Pinlei, shangdianjia: Option<i64>) -> Wupin {
    Wupin {
        id: id.into(),
        mingcheng: mingcheng.into(),
        pinlei,
        dui: None,
        shangdianjia,
        laiyuan: None,
    }
}

/// PLAN §6.4 的配方：材料 800 金、精力 100（1 点 = 2 金）、制作费 0 → 总成本 1000 金；
/// 产出 1 件、售价 1200 金。
fn plan_catalog() -> Catalog {
    Catalog {
        banben: Banben {
            level: 130,
            season: 3,
            modified: 20261112,
            note: String::new(),
        },
        wupin: vec![
            wupin("5_cailiao", "材料", Pinlei::Cailiao, None),
            wupin("5_chengpin", "成品", Pinlei::Chengpin, None),
            wupin("5_stone_low", "五行石（低）", Pinlei::Wuxingshi, None),
            wupin("5_stone_high", "五行石（高）", Pinlei::Wuxingshi, None),
        ],
        peifang: vec![Peifang {
            id: "plan_recipe".into(),
            mingcheng: "§6.4 示例配方".into(),
            jiyi: Jiyi::Pengren,
            dengji: 50,
            jingli: 100,
            zhizuofei: 0,
            cailiao: vec![Cailiao {
                wupin_id: "5_cailiao".into(),
                shuliang: 1,
            }],
            chuchan: vec![Chuchan {
                wupin_id: "5_chengpin".into(),
                min: 1,
                max: 1,
                quanzhong: QUANZHONG_FENMU,
            }],
            laiyuan: None,
        }],
        wuxingshi: vec![Zhuanhuan {
            id: "wuxing_1to3".into(),
            mingcheng: "一级→三级（13 合 1）".into(),
            cong: "5_stone_low".into(),
            dao: "5_stone_high".into(),
            xuyao: 13,
            jingli: 10,
            jinbi: 3_900,
            chenggonglv_bp: 10_000,
            laiyuan: None,
        }],
        fuwuqi: vec![],
        zhiji: vec![],
    }
}

fn plan_prices() -> BTreeMap<String, i64> {
    BTreeMap::from([
        ("5_cailiao".to_string(), 8_000_000),   // 800 金
        ("5_chengpin".to_string(), 12_000_000), // 1200 金
    ])
}

#[test]
fn plan_6_4_manual_net() {
    // 售价 1200 金：手续费 5% = 60 金（HALF_UP），保管费 80 银 → 到手 1139 金 20 银
    let req = NetRequest {
        shoujia: 12_000_000,
        shuliang: 1,
        shouxufei: cfg(),
        guadan_cishu: 1,
    };
    let r = fee::jisuan_net(&req).expect("净收入计算");
    assert_eq!(r.zonge, 12_000_000);
    assert_eq!(r.feiyong, 600_000);
    assert_eq!(r.baoguanfei, 8_000);
    assert_eq!(r.daoshou, 11_392_000);
    assert_eq!(money::format_tong(r.zonge), "1200金");
    assert_eq!(money::format_tong(r.feiyong), "60金");
    assert_eq!(money::format_tong(r.daoshou), "1139金20银");
    // 净收入（扣手续费、未扣保管费）= 1140 金，符合 PLAN §6.4 描述
    assert_eq!(r.zonge - r.feiyong, 11_400_000);
}

#[test]
fn plan_6_4_recipe_profit() {
    let catalog = plan_catalog();
    let req = JisuanRequest {
        peifang_id: "plan_recipe".into(),
        cishu: 1,
        guadan_cishu: 1,
        jingli_danjia: 20_000, // 1 点精力 = 2 金
        shouxufei: cfg(),
        jiage: plan_prices(),
        jiyi_dengji: 0,
        zhongjian_zizhi: true,
        shoudong: BTreeSet::new(),
    };
    let r = craft::jisuan(&catalog, &req).expect("配方利润计算");
    // 成本构成：材料 800 金 + 精力 200 金 = 1000 金；保管费 80 银
    assert_eq!(r.chengben_cailiao, 8_000_000);
    assert_eq!(r.chengben_jingli, 2_000_000);
    assert_eq!(r.zhizuofei, 0);
    assert_eq!(r.baoguanfei, 8_000);
    assert_eq!(r.zong_chengben, 10_008_000);
    // 收入 1200 金；手续费 60 金；净利润 = 1140 − 0.8 保管费 − 1000 = 139.2 金
    assert_eq!(r.shouru, 12_000_000);
    assert_eq!(r.jingshouru, 11_400_000);
    assert_eq!(r.lirun, 1_392_000);
    // 利润率 ≈ 13.9%（分母为含保管费的总成本，1391 万分比）
    assert_eq!(r.lirunlv_bp, 1_391);
    // 每精力利润 = 139.2 金 / 100 = 1.392 金 = 13_920 铜
    assert_eq!(r.jingli, 100);
    assert_eq!(r.mei_jingli, 13_920);
    assert!((r.chuchan_qiwang - 1.0).abs() < 1e-9);
    assert_eq!(money::format_tong(r.lirun), "139金20银"); // 139.2 金
    assert!(r.quejia.is_empty());
    // 保本单价（1 件）：总成本 10,008,000 → 5% 后覆盖的最小售价 10,535,000? 由 fee 层精确给出
    assert_eq!(r.baobenjia, fee::baobenjia(10_008_000, &cfg(), 0).unwrap());

    // 批量 3 次：所有收支按期望线性放大
    let mut batch = req.clone();
    batch.cishu = 3;
    let b = craft::jisuan(&catalog, &batch).expect("批量计算");
    assert_eq!(b.zong_chengben, r.zong_chengben * 3);
    assert_eq!(b.shouru, r.shouru * 3);
    assert_eq!(b.jingshouru, r.jingshouru * 3);
    assert_eq!(b.lirun, r.lirun * 3);
}

#[test]
fn plan_6_2_baoben_minimal() {
    // 保本价 = 满足「售价 − 手续费 − 保管费 >= 成本」的最小售价；逐铜验证 ±1。
    let cost = 8_000_000; // 800 金
    let p = fee::baobenjia(cost, &cfg(), 1).expect("保本价");
    let net = |price: i64| price - fee::jisuan_shouxufei(price, &cfg()) - cfg().baoguanfei;
    assert!(net(p) >= cost);
    assert!(net(p - 1) < cost);
    // 0% 费率：保本价 = 成本 + 保管费
    let mut free_fee = cfg();
    free_fee.shuilv_bp = 0;
    assert_eq!(fee::baobenjia(cost, &free_fee, 2).unwrap(), cost + 16_000);
}

#[test]
fn money_roundtrip_negative_profit() {
    // 负利润（-139金20银）也要 format → parse 往返一致
    let loss = -1_392_000_i64;
    let text = money::format_tong(loss);
    assert_eq!(text, "负139金20银");
    assert_eq!(money::parse_jin(&text).expect("负数解析"), loss);
    // 极值不 panic 且往返
    for v in [
        0,
        1,
        99,
        100,
        10_000,
        100_000_000,
        -12_003,
        i64::MAX,
        i64::MIN,
    ] {
        assert_eq!(
            money::parse_jin(&money::format_tong(v)).unwrap(),
            v,
            "v={v}"
        );
    }
}

#[test]
fn convert_expected_cost_1_over_p() {
    let catalog = plan_catalog();
    let mut req = ZhuanhuanRequest {
        zhuanhuan_id: "wuxing_1to3".into(),
        cishu: 1,
        jingli_danjia: 0,
        shouxufei: cfg(),
        jiage: BTreeMap::from([
            ("5_stone_low".to_string(), 100),    // 低阶石 100 铜
            ("5_stone_high".to_string(), 6_000), // 高阶石 6000 铜
        ]),
    };
    // 成功率 100%：单次成本 = 13×100 + 3900 = 5200；收入 6000；手续费 300 → 利润 500
    let r = convert::jisuan(&catalog, &req).expect("转换计算");
    assert!((r.qiwang_cishu - 1.0).abs() < 1e-9);
    assert_eq!(r.zong_chengben, 5_200);
    assert_eq!(r.shouru, 6_000);
    assert_eq!(r.lirun, 500); // 6000 − 手续费 300 − 成本 5200
    // 目标 3 个：期望投入仍 3 次（p=100%）
    req.cishu = 3;
    let r = convert::jisuan(&catalog, &req).expect("转换批量");
    assert!((r.qiwang_cishu - 3.0).abs() < 1e-9);
    assert_eq!(r.zong_chengben, 15_600);
    assert_eq!(r.shouru, 18_000);
    // 成功率 50%：每个产出的期望投入翻倍
    let mut half = catalog.clone();
    if let Some(z) = half.wuxingshi.first_mut() {
        z.chenggonglv_bp = 5_000;
    }
    req.cishu = 1;
    let r = convert::jisuan(&half, &req).expect("50% 转换");
    assert!((r.qiwang_cishu - 2.0).abs() < 1e-9);
    assert_eq!(r.zong_chengben, 10_400);
}

#[test]
fn rank_default_excludes_missing_price() {
    let catalog = plan_catalog();
    let req = PaihangRequest {
        jiyi: None,
        jingli_danjia: 20_000, // 1 点精力 = 2 金
        shouxufei: cfg(),
        jiage: plan_prices(),
        zhibiao: PaihangZhibiao::Lirun,
        jiyi_dengji: 0,
        limit: Some(10),
        baohan_quejia: false,
        zhongjian_zizhi: true,
        baohan_zhongjian: false,
        shoudong: BTreeSet::new(),
    };
    let rows = rank::paihang(&catalog, &req);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].peifang_id, "plan_recipe");
    assert!(!rows[0].quejia);
    assert_eq!(rows[0].lirun, 1_392_000);
    // 不含成品价 → 缺价 → 默认排除，limit 后仍为空
    let mut missing = req.clone();
    missing.jiage.remove("5_chengpin");
    assert!(rank::paihang(&catalog, &missing).is_empty());
    // 显式要求包含缺价：保留并标记 quejia，且排在有效行之后
    missing.baohan_quejia = true;
    let rows = rank::paihang(&catalog, &missing);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].quejia);
}

#[test]
fn plan_6_3_1_middle_product_self_made() {
    // PLAN §6.3.1：中间产物不按交易行价，而是自制展开（材料递归 + 精力 + 制作费，按期望产出折算）
    let mid_recipe = Peifang {
        id: "mid_recipe".into(),
        mingcheng: "中间产物配方".into(),
        jiyi: Jiyi::Pengren,
        dengji: 10,
        jingli: 10,
        zhizuofei: 500,
        cailiao: vec![
            Cailiao {
                wupin_id: "5_raw_a".into(),
                shuliang: 2,
            },
            Cailiao {
                wupin_id: "5_raw_b".into(),
                shuliang: 1,
            },
        ],
        chuchan: vec![Chuchan {
            wupin_id: "5_mid".into(),
            min: 2,
            max: 3,
            quanzhong: QUANZHONG_FENMU,
        }],
        laiyuan: None,
    };
    let final_recipe = Peifang {
        id: "final_recipe".into(),
        mingcheng: "最终配方".into(),
        jiyi: Jiyi::Pengren,
        dengji: 20,
        jingli: 20,
        zhizuofei: 0,
        cailiao: vec![Cailiao {
            wupin_id: "5_mid".into(),
            shuliang: 3,
        }],
        chuchan: vec![Chuchan {
            wupin_id: "5_out".into(),
            min: 1,
            max: 1,
            quanzhong: QUANZHONG_FENMU,
        }],
        laiyuan: None,
    };
    let catalog = Catalog {
        banben: Banben::default(),
        wupin: vec![
            wupin("5_raw_a", "原料A", Pinlei::Cailiao, Some(100)),
            wupin("5_raw_b", "原料B", Pinlei::Cailiao, Some(150)),
            wupin("5_mid", "中间产物", Pinlei::Cailiao, None),
            wupin("5_out", "成品", Pinlei::Chengpin, None),
        ],
        peifang: vec![mid_recipe, final_recipe],
        wuxingshi: vec![],
        fuwuqi: vec![],
        zhiji: vec![],
    };
    let req = JisuanRequest {
        peifang_id: "final_recipe".into(),
        cishu: 1,
        guadan_cishu: 1,
        jingli_danjia: 10,
        shouxufei: cfg(),
        jiage: BTreeMap::from([
            ("5_raw_a".to_string(), 100),
            ("5_raw_b".to_string(), 200),
            ("5_mid".to_string(), 999), // 行情价不参与中间产物成本（默认自制）
            ("5_out".to_string(), 5_000),
        ]),
        jiyi_dengji: 0,
        zhongjian_zizhi: true,
        shoudong: BTreeSet::new(),
    };
    let r = craft::jisuan(&catalog, &req).expect("中间产物展开计算");
    // 中间产物单位成本 = (2×100 + 200 + 10×10 + 500) / 2.5 = 400
    assert_eq!(r.chengben_cailiao, 1_200);
    assert_eq!(r.chengben_zhongjian, 1_200);
    assert_eq!(r.jingli_zhongjian, 12);
    assert_eq!(r.chengben_jingli, 200);
    assert_eq!(r.baoguanfei, 8_000);
    assert_eq!(
        r.zong_chengben,
        r.chengben_cailiao + r.chengben_jingli + r.zhizuofei + r.baoguanfei
    );
    assert_eq!(r.zong_chengben, 9_400);
    assert_eq!(r.zhongjian.len(), 1);
    assert_eq!(r.zhongjian[0].peifang_id, "mid_recipe");
    assert_eq!(r.zhongjian[0].danjia, 400);
    assert_eq!(r.zhongjian[0].jingli, 12);
    // 关闭开关 = 旧行为：中间产物按行情价 999 计
    let mut old = req.clone();
    old.zhongjian_zizhi = false;
    let r_old = craft::jisuan(&catalog, &old).expect("旧行为计算");
    assert_eq!(r_old.chengben_cailiao, 2_997);
    assert!(r_old.zhongjian.is_empty());
    assert_eq!(r_old.jingli_zhongjian, 0);
}
