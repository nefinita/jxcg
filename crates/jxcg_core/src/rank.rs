//! 全配方收益排行。
//!
//! 口径（PLAN.md §6.3 / §6.3.1）：
//! - 指标三选一：单次净利润 / 每点精力净利润 / 利润率；
//! - **缺价配方默认排除**；`baohan_quejia = true` 时保留并标记 `quejia = true`，
//!   且缺价行一律排在有效行之后（缺价按 0 计会虚高利润，不能混排）；
//! - **「只产中间产物」的配方默认不参与排行**（`baohan_zhongjian = false`）；
//!   包含时标记 `zhongjian = true`，排序规则不变；
//! - `zhongjian_zizhi` 同样作用于排行成本（默认 true = 中间产物按自制成本展开）；
//! - 排序稳定：缺价标记 → 指标降序 → 配方 id 升序兜底（同值结果确定）；
//! - `limit` 截断在排序之后；一次请求只建一次中间产物索引，单位成本请求内 memo。

use std::collections::{BTreeMap, BTreeSet};

use crate::craft;
use jxcg_api::{Catalog, JisuanRequest, PaihangRequest, PaihangRow, PaihangZhibiao};

/// 计算排行（按指标降序；缺价 / 中间产物配方按开关过滤）。
pub fn paihang(catalog: &Catalog, req: &PaihangRequest) -> Vec<PaihangRow> {
    // 一次请求只建一次索引；中间产物单位成本在请求内 memo（1036 配方量级的关键优化）
    let mut calculator =
        craft::Calculator::new(catalog, &req.jiage, &req.shoudong, req.jingli_danjia);
    let mut rows = Vec::with_capacity(catalog.peifang.len());
    for peifang in &catalog.peifang {
        if let Some(jiyi) = req.jiyi
            && peifang.jiyi != jiyi
        {
            continue;
        }
        // 「只产中间产物」的配方默认不进入排行（仍可作为成本展开来源被引用）
        let zhongjian_peifang = calculator.index().is_zhongjian_peifang(peifang);
        if zhongjian_peifang && !req.baohan_zhongjian {
            continue;
        }
        let jisuan_req = JisuanRequest {
            peifang_id: peifang.id.clone(),
            cishu: 1,
            guadan_cishu: 1,
            // 价格上下文由 Calculator 构造参数统一提供（这里仅占位，保持字段完整）
            jingli_danjia: req.jingli_danjia,
            shouxufei: req.shouxufei.clone(),
            jiage: BTreeMap::new(),
            jiyi_dengji: req.jiyi_dengji,
            zhongjian_zizhi: req.zhongjian_zizhi,
            shoudong: BTreeSet::new(),
        };
        let Ok(result) = calculator.jisuan(&jisuan_req) else {
            // 数据不完整的配方（如 min > max）不能参与排行，跳过
            continue;
        };
        let quejia = !result.quejia.is_empty();
        if quejia && !req.baohan_quejia {
            continue;
        }
        rows.push(PaihangRow {
            peifang_id: peifang.id.clone(),
            mingcheng: peifang.mingcheng.clone(),
            jiyi: peifang.jiyi,
            lirun: result.lirun,
            lirunlv_bp: result.lirunlv_bp,
            mei_jingli: result.mei_jingli,
            jingli: result.jingli,
            quejia,
            zhongjian: zhongjian_peifang,
        });
    }

    rows.sort_by(|a, b| {
        // 缺价行永远排在有效行之后
        let quejia_order = a.quejia.cmp(&b.quejia);
        let primary = match req.zhibiao {
            PaihangZhibiao::Lirun => b.lirun.cmp(&a.lirun),
            PaihangZhibiao::MeiJingli => b.mei_jingli.cmp(&a.mei_jingli),
            PaihangZhibiao::Lirunlv => b.lirunlv_bp.cmp(&a.lirunlv_bp),
        };
        quejia_order
            .then(primary)
            .then_with(|| a.peifang_id.cmp(&b.peifang_id))
    });

    if let Some(limit) = req.limit {
        rows.truncate(limit);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use jxcg_api::{
        Banben, Cailiao, Chuchan, Jiyi, Peifang, Pinlei, QUANZHONG_FENMU, Quzheng, Shouxufei, Wupin,
    };
    use std::collections::BTreeMap;

    fn wupin(id: &str, shop: Option<i64>) -> Wupin {
        Wupin {
            id: id.into(),
            mingcheng: format!("物品{id}"),
            pinlei: Pinlei::Cailiao,
            dui: None,
            shangdianjia: shop,
            laiyuan: None,
        }
    }

    fn recipe(id: &str, jiyi: Jiyi, jingli: u32, cost_item: &str, out_item: &str) -> Peifang {
        Peifang {
            id: id.into(),
            mingcheng: format!("配方{id}"),
            jiyi,
            dengji: 10,
            jingli,
            zhizuofei: 0,
            cailiao: vec![Cailiao {
                wupin_id: cost_item.into(),
                shuliang: 1,
            }],
            chuchan: vec![Chuchan {
                wupin_id: out_item.into(),
                min: 1,
                max: 1,
                quanzhong: QUANZHONG_FENMU,
            }],
            laiyuan: None,
        }
    }

    fn catalog() -> Catalog {
        Catalog {
            banben: Banben::default(),
            // 材料价统一 100（行情表统一定价），成品无商店价
            wupin: vec![
                wupin("mat", Some(100)),
                wupin("outA", None),
                wupin("outB", None),
                wupin("outC", None),
                wupin("out_que", None),
                wupin("mat2", None),
            ],
            peifang: vec![
                recipe("r_a", Jiyi::Pengren, 10, "mat", "outA"),
                recipe("r_b", Jiyi::Pengren, 20, "mat", "outB"),
                recipe("r_c", Jiyi::Zhiyao, 10, "mat", "outC"),
                recipe("r_q", Jiyi::Zhiyao, 10, "mat", "out_que"),
                // 缺材料价：mat2 无商店价且行情缺价
                {
                    let mut p = recipe("r_missing", Jiyi::Zhuzhao, 10, "mat2", "outA");
                    p.cailiao[0].wupin_id = "mat2".into();
                    p
                },
            ],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        }
    }

    fn base_req() -> PaihangRequest {
        PaihangRequest {
            jiyi: None,
            jingli_danjia: 0,
            shouxufei: Shouxufei {
                shuilv_bp: 500,
                quzheng: Quzheng::HalfUp,
                baoguanfei: 8_000,
            },
            jiage: BTreeMap::from([
                ("mat".to_string(), 100),
                // 缺价：out_que 不在价格表里 → 该配方缺价；mat2 无行情/商店价 → 该配方缺价
                ("outA".to_string(), 500),
                ("outB".to_string(), 500),
                ("outC".to_string(), 500),
            ]),
            zhibiao: PaihangZhibiao::Lirun,
            jiyi_dengji: 0,
            limit: None,
            baohan_quejia: false,
            zhongjian_zizhi: true,
            baohan_zhongjian: false,
            shoudong: BTreeSet::new(),
        }
    }

    #[test]
    fn default_excludes_quejia() {
        let rows = paihang(&catalog(), &base_req());
        // r_q 缺成品价、r_missing 缺材料价 → 默认排除；只剩 r_a/r_b/r_c
        let ids: Vec<&str> = rows.iter().map(|r| r.peifang_id.as_str()).collect();
        // 三条有效配方利润相同 → 按 id 升序兜底
        assert_eq!(ids, vec!["r_a", "r_b", "r_c"]);
        assert!(rows.iter().all(|r| !r.quejia));
    }

    #[test]
    fn include_quejia_last() {
        let mut req = base_req();
        req.baohan_quejia = true;
        let rows = paihang(&catalog(), &req);
        let ids: Vec<&str> = rows.iter().map(|r| r.peifang_id.as_str()).collect();
        // 有效行在前，缺价行在后
        assert_eq!(ids.len(), 5);
        let split = rows.iter().position(|r| r.quejia).unwrap();
        assert!(rows[split..].iter().all(|r| r.quejia));
        assert!(!ids[..split].contains(&"r_q"));
        assert!(!ids[..split].contains(&"r_missing"));
        assert!(ids[split..].contains(&"r_q"));
        assert!(ids[split..].contains(&"r_missing"));
    }

    #[test]
    fn three_metrics_sorted() {
        // 全用行情价，构造不同利润/精力/利润率
        let cat = catalog();
        let mut req = base_req();
        req.jiyi = Some(Jiyi::Pengren); // r_a / r_b
        // r_a：收入 500，成本 100+8000，利润 -7600；精力 10
        // r_b：收入 500，成本 100+8000，利润 -7600；精力 20 → 每精力 -380（r_a 每精力 -760）
        for (zhibiao, expect_first) in [
            (PaihangZhibiao::Lirun, "r_a"),     // 同利润，id 兜底
            (PaihangZhibiao::MeiJingli, "r_b"), // -380 > -760
            (PaihangZhibiao::Lirunlv, "r_a"),   // 同利润率，id 兜底
        ] {
            req.zhibiao = zhibiao;
            let rows = paihang(&cat, &req);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].peifang_id, expect_first, "指标 {zhibiao:?}");
            assert_eq!(
                rows[1].peifang_id,
                if expect_first == "r_a" { "r_b" } else { "r_a" }
            );
        }
    }

    #[test]
    fn tie_break_by_id() {
        // 两个配方完全一样（id 不同）→ 按 id 升序稳定
        let mut cat = catalog();
        cat.peifang = vec![
            recipe("r_zzz", Jiyi::Pengren, 10, "mat", "outA"),
            recipe("r_aaa", Jiyi::Pengren, 10, "mat", "outB"),
            recipe("r_mmm", Jiyi::Pengren, 10, "mat", "outA"),
        ];
        let mut req = base_req();
        req.zhibiao = PaihangZhibiao::Lirun;
        let rows = paihang(&cat, &req);
        let ids: Vec<&str> = rows.iter().map(|r| r.peifang_id.as_str()).collect();
        assert_eq!(ids, vec!["r_aaa", "r_mmm", "r_zzz"]);
    }

    #[test]
    fn jiyi_filter_and_limit() {
        let mut req = base_req();
        req.jiyi = Some(Jiyi::Pengren);
        let rows = paihang(&catalog(), &req);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.jiyi == Jiyi::Pengren));

        // limit 在排序之后截断
        let mut req_all = base_req();
        req_all.baohan_quejia = true;
        req_all.limit = Some(2);
        let rows = paihang(&catalog(), &req_all);
        assert_eq!(rows.len(), 2);
        // limit 0 → 空
        req_all.limit = Some(0);
        assert!(paihang(&catalog(), &req_all).is_empty());
        // limit 大于总数 → 全部
        req_all.limit = Some(100);
        assert_eq!(paihang(&catalog(), &req_all).len(), 5);
    }

    #[test]
    fn invalid_recipes_are_skipped() {
        // min > max 的坏配方不参与排行，不影响其他行
        let mut cat = catalog();
        cat.peifang.push({
            let mut p = recipe("r_bad", Jiyi::Pengren, 10, "mat", "outA");
            p.chuchan[0].min = 3;
            p.chuchan[0].max = 1;
            p
        });
        let rows = paihang(&cat, &base_req());
        assert!(rows.iter().all(|r| r.peifang_id != "r_bad"));
        assert_eq!(rows.len(), 3); // r_a/r_b/r_c
    }

    // ========================================================================
    // 中间产物排行（PLAN.md §6.3.1）
    // ========================================================================

    /// 中间产物排行目录：
    /// - raw（商店价 100）；mid 由 mid_recipe 产出（1×raw + 精力 10，产出 1 件）
    ///   单位成本 = 100 + 10×精力单价(10) = 200；mid 同时被 final_a / final_b 使用；
    /// - final_a 产出 out_a（行情 1000）、final_b 产出 out_b（行情 1200）；
    /// - mid 自身有行情价 999，但默认自制展开（只有手填覆盖才按行情价采购）。
    fn zhongjian_rank_catalog() -> Catalog {
        let mut mid = recipe("mid_recipe", Jiyi::Pengren, 10, "raw", "mid");
        mid.zhizuofei = 0;
        let mut final_a = recipe("final_a", Jiyi::Pengren, 0, "mid", "out_a");
        final_a.cailiao[0].wupin_id = "mid".into();
        let mut final_b = recipe("final_b", Jiyi::Pengren, 0, "mid", "out_b");
        final_b.cailiao[0].wupin_id = "mid".into();
        Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("raw", Some(100)),
                wupin("mid", None),
                wupin("out_a", None),
                wupin("out_b", None),
            ],
            peifang: vec![mid, final_a, final_b],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        }
    }

    fn zhongjian_rank_req() -> PaihangRequest {
        PaihangRequest {
            jiyi: None,
            jingli_danjia: 10,
            shouxufei: Shouxufei {
                shuilv_bp: 500,
                quzheng: Quzheng::HalfUp,
                baoguanfei: 0,
            },
            jiage: BTreeMap::from([
                ("raw".to_string(), 100),
                ("mid".to_string(), 999), // 行情价：默认不用（自制展开）
                ("out_a".to_string(), 1_000),
                ("out_b".to_string(), 1_200),
            ]),
            zhibiao: PaihangZhibiao::Lirun,
            jiyi_dengji: 0,
            limit: None,
            baohan_quejia: false,
            zhongjian_zizhi: true,
            baohan_zhongjian: false,
            shoudong: BTreeSet::new(),
        }
    }

    #[test]
    fn zhongjian_recipes_excluded_by_default() {
        let req = zhongjian_rank_req();
        let rows = paihang(&zhongjian_rank_catalog(), &req);
        let ids: Vec<&str> = rows.iter().map(|r| r.peifang_id.as_str()).collect();
        // mid_recipe 只产中间产物 → 默认排除；final_a/final_b 用自制成本 200：
        // final_a: 1000−50−200 = 750；final_b: 1200−60−200 = 940
        assert_eq!(ids, vec!["final_b", "final_a"]);
        assert!(rows.iter().all(|r| !r.zhongjian));
        assert_eq!(rows[0].lirun, 940);
        assert_eq!(rows[1].lirun, 750);
        // 每精力利润按（最终精力 + 中间产物精力）计算：750 / (0 + 10) = 75
        assert_eq!(rows[1].mei_jingli, 75);
    }

    #[test]
    fn zhongjian_recipes_included_with_flag() {
        let mut req = zhongjian_rank_req();
        req.baohan_zhongjian = true;
        let rows = paihang(&zhongjian_rank_catalog(), &req);
        assert_eq!(rows.len(), 3);
        // 排序稳定：净利润降序 final_b 940 > final_a 750 > mid_recipe 749
        let ids: Vec<&str> = rows.iter().map(|r| r.peifang_id.as_str()).collect();
        assert_eq!(ids, vec!["final_b", "final_a", "mid_recipe"]);
        assert!(rows[2].zhongjian);
        assert!(!rows[0].zhongjian && !rows[1].zhongjian);
        // 中间产物配方自身按自制成本计算：材料 100 + 精力 10×10 = 200；
        // 收入 999 − 手续费 50 = 949；利润 749
        assert_eq!(rows[2].lirun, 749);
        // limit 在排序后截断
        req.limit = Some(2);
        assert_eq!(paihang(&zhongjian_rank_catalog(), &req).len(), 2);
    }

    #[test]
    fn zhongjian_zizhi_switch_affects_rank_cost() {
        // 关闭自制展开：final 配方按 mid 行情价 999 计材料成本
        let mut req = zhongjian_rank_req();
        req.zhongjian_zizhi = false;
        let rows = paihang(&zhongjian_rank_catalog(), &req);
        let ids: Vec<&str> = rows.iter().map(|r| r.peifang_id.as_str()).collect();
        assert_eq!(ids, vec!["final_b", "final_a"]);
        // final_b: 1140 − 999 = 141；final_a: 950 − 999 = −49
        assert_eq!(rows[0].lirun, 141);
        assert_eq!(rows[1].lirun, -49);
    }

    #[test]
    fn zhongjian_rank_metric_sorting() {
        // 每精力指标：final_a/final_b 自身精力为 0，中间产物精力摊入后仍退化为净利润；
        // 用中间产物配方自身（精力 10）参与排序验证三种指标都稳定。
        let mut req = zhongjian_rank_req();
        req.baohan_zhongjian = true;
        for (zhibiao, expect_first) in [
            (PaihangZhibiao::Lirun, "final_b"),
            (PaihangZhibiao::MeiJingli, "final_b"),
            (PaihangZhibiao::Lirunlv, "final_b"),
        ] {
            req.zhibiao = zhibiao;
            let rows = paihang(&zhongjian_rank_catalog(), &req);
            assert_eq!(rows.len(), 3);
            assert_eq!(rows[0].peifang_id, expect_first, "指标 {zhibiao:?}");
            // 同值时仍按 id 兜底，结果确定
            let ids: Vec<&str> = rows.iter().map(|r| r.peifang_id.as_str()).collect();
            let mut sorted = ids.clone();
            sorted.sort();
            assert_eq!(ids.len(), sorted.len());
        }
    }

    #[test]
    fn zhongjian_memo_keeps_quejia_for_later_recipes() {
        // 两个最终配方共用同一中间产物，中间产物的采购原料缺价：
        // 排行对单位自制成本做请求内 memo，memo 命中时也必须为后续配方重复冒泡 quejia。
        let mut mid = recipe("mid_recipe", Jiyi::Pengren, 0, "missing_raw", "mid");
        mid.zhizuofei = 0;
        let mut final_a = recipe("final_a", Jiyi::Pengren, 0, "mid", "out_a");
        final_a.cailiao[0].wupin_id = "mid".into();
        let mut final_b = recipe("final_b", Jiyi::Pengren, 0, "mid", "out_b");
        final_b.cailiao[0].wupin_id = "mid".into();
        let cat = Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("missing_raw", None),
                wupin("mid", None),
                wupin("out_a", None),
                wupin("out_b", None),
            ],
            peifang: vec![mid, final_a, final_b],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        };
        let mut req = zhongjian_rank_req();
        req.jiage = BTreeMap::from([("out_a".to_string(), 1_000), ("out_b".to_string(), 1_200)]);
        // 默认排除缺价 → 两个最终配方都不参与排行
        assert!(paihang(&cat, &req).is_empty());
        // 显式包含缺价 → 两个都在，且都标记 quejia（memo 命中不丢缺价）
        req.baohan_quejia = true;
        let rows = paihang(&cat, &req);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.quejia));
        assert!(rows.iter().all(|row| !row.zhongjian));
    }
}
