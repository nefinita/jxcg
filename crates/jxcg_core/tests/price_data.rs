//! 真实随包数据上的「NPC 商店价优先」联调测试。
//!
//! 用户明确要求：料酒（5_31974，商店 6000 铜）、今朝醉（5_32014，商店 1200000 铜）等
//! 官方 NPC 无限量售卖的材料不再按交易行价算成本；手填仍最高优先。

use std::collections::{BTreeMap, BTreeSet};

use jxcg_api::{Catalog, JisuanRequest, Quzheng, Shouxufei};
use jxcg_core::{craft, store};

fn repo_catalog() -> Catalog {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    store::load_catalog(&dir).expect("data/ 应能加载")
}

fn cfg() -> Shouxufei {
    Shouxufei {
        shuilv_bp: 500,
        quzheng: Quzheng::HalfUp,
        baoguanfei: 8_000,
    }
}

/// 取第一个使用指定材料的真实配方 id。
fn peifang_using(catalog: &Catalog, wupin_id: &str) -> String {
    catalog
        .peifang
        .iter()
        .find(|peifang| peifang.cailiao.iter().any(|item| item.wupin_id == wupin_id))
        .map(|peifang| peifang.id.clone())
        .unwrap_or_else(|| panic!("真实数据里应有使用 {wupin_id} 的配方"))
}

fn request(
    peifang_id: &str,
    jiage: BTreeMap<String, i64>,
    shoudong: BTreeSet<String>,
) -> JisuanRequest {
    JisuanRequest {
        peifang_id: peifang_id.into(),
        cishu: 1,
        guadan_cishu: 0,
        jingli_danjia: 0,
        shouxufei: cfg(),
        jiage,
        jiyi_dengji: 0,
        zhongjian_zizhi: true,
        shoudong,
    }
}

#[test]
fn npc_item_uses_shop_price_on_real_data() {
    let catalog = repo_catalog();
    // 料酒：NPC 商店价 0.6 金（6000 铜）
    let wupin = catalog
        .wupin
        .iter()
        .find(|w| w.id == "5_31974")
        .expect("数据里应有料酒 5_31974");
    assert_eq!(wupin.shangdianjia, Some(6_000));

    let peifang_id = peifang_using(&catalog, "5_31974");
    // 行情表里故意给 1 铜（远低于商店价）→ 仍应按商店价 6000 计
    let jiage = BTreeMap::from([("5_31974".to_string(), 1)]);
    let r = craft::jisuan(&catalog, &request(&peifang_id, jiage, BTreeSet::new()))
        .expect("真实配方应能计算");
    let row = r
        .cailiao
        .iter()
        .find(|item| item.wupin_id == "5_31974")
        .expect("材料明细应含料酒");
    assert_eq!(row.danjia, 6_000);
    assert_eq!(row.laiyuan, "shangdian");

    // 手填覆盖（shoudong + jiage）最高优先：按手填 1 铜计
    let jiage = BTreeMap::from([("5_31974".to_string(), 1)]);
    let shoudong = BTreeSet::from(["5_31974".to_string()]);
    let r =
        craft::jisuan(&catalog, &request(&peifang_id, jiage, shoudong)).expect("真实配方应能计算");
    let row = r
        .cailiao
        .iter()
        .find(|item| item.wupin_id == "5_31974")
        .expect("材料明细应含料酒");
    assert_eq!(row.danjia, 1);
    assert_eq!(row.laiyuan, "shoudong");
}

#[test]
fn shop_items_count_matches_data_contract() {
    // 当前过滤后数据共 44 个 NPC 有售物品（task-14 剔除旧家园家具后）；数据更新时同步此断言
    let catalog = repo_catalog();
    let count = catalog
        .wupin
        .iter()
        .filter(|w| w.shangdianjia.unwrap_or(0) > 0)
        .count();
    assert_eq!(
        count, 44,
        "NPC 有售物品数量应为 44（task-14 数据重导后同步）"
    );
}
