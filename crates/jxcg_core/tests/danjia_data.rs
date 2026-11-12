//! 真实数据回归：多产出配方的「单件成本 = 每批成本 ÷ 期望产出」+ 不可获得材料分类。
//!
//! 背景：用户反馈「中间产物价格导入最终产物时没有除期望产出」。核实计算本身没错
//! （百花布 batch 667,000 / 2.5 = 266,800；木板 batch 335,000 / 2.5 = 134,000），
//! 但结果里只有总成本容易误读，故补 `JisuanResult.danjia_chengben` 并用真实数据钉死除法。

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

fn request(peifang_id: &str, jiage: BTreeMap<String, i64>) -> JisuanRequest {
    JisuanRequest {
        peifang_id: peifang_id.into(),
        cishu: 1,
        guadan_cishu: 0,
        jingli_danjia: 0,
        shouxufei: cfg(),
        jiage,
        jiyi_dengji: 0,
        zhongjian_zizhi: true,
        shoudong: BTreeSet::new(),
    }
}

#[test]
fn muban_batch_cost_divided_by_expected_output() {
    let catalog = repo_catalog();
    // 木板 furniture_58：5×木料 + 5×清漆（NPC 商店 60,000）+ 5×绳索（NPC 商店 6,000），
    // 每次期望产出 2.5 件（min 2 / max 3）；木料按 1,000 铜补价。
    // batch = 5,000 + 300,000 + 30,000 = 335,000 → 单件 335,000 / 2.5 = 134,000
    let jiage = BTreeMap::from([("5_35566".to_string(), 1_000)]);
    let r = craft::jisuan(&catalog, &request("furniture_58", jiage.clone())).expect("木板应可计算");
    assert_eq!(r.peifang_mingcheng, "木板");
    assert!((r.chuchan_qiwang - 2.5).abs() < 1e-9);
    assert_eq!(r.chengben_cailiao, 335_000);
    assert_eq!(r.zong_chengben, 335_000);
    assert_eq!(r.danjia_chengben, 134_000);
    // 单件成本 × 期望产出 = 总成本（本例整除）
    assert_eq!(r.danjia_chengben as f64 * 2.5, r.zong_chengben as f64);

    // 批量 3 次：总成本 ×3、期望产出 7.5，单件成本不变
    let mut batch = request("furniture_58", jiage);
    batch.cishu = 3;
    let b = craft::jisuan(&catalog, &batch).expect("木板批量应可计算");
    assert_eq!(b.zong_chengben, 1_005_000);
    assert!((b.chuchan_qiwang - 7.5).abs() < 1e-9);
    assert_eq!(b.danjia_chengben, 134_000);
}

#[test]
fn baihuabu_middle_product_unit_cost_divided() {
    let catalog = repo_catalog();
    // 百花布 tailoring_1285：5×青魄玉 + 2×玛瑙（各补价 1,000）+ 1×百股线(300,000)
    // + 10×百染线(30,000) + 10×棉线(6,000) = 667,000；期望产出 2.5 → 单位成本 266,800
    let jiage = BTreeMap::from([
        ("5_89298".to_string(), 1_000),
        ("5_30846".to_string(), 1_000),
    ]);
    // 找一个直接使用百花布的真实成品配方
    let user = catalog
        .peifang
        .iter()
        .find(|peifang| {
            peifang
                .cailiao
                .iter()
                .any(|item| item.wupin_id == "5_89956")
        })
        .expect("数据里应有使用百花布的配方");
    let r = craft::jisuan(&catalog, &request(&user.id, jiage)).expect("成品配方应可计算");
    let row = r
        .cailiao
        .iter()
        .find(|row| row.wupin_id == "5_89956")
        .expect("材料表应含百花布");
    assert!(row.zhongjian, "百花布行应为自制展开");
    assert_eq!(row.laiyuan, "zizhi");
    assert_eq!(row.danjia, 266_800); // 已除期望产出 2.5
    let zj = r
        .zhongjian
        .iter()
        .find(|item| item.wupin_id == "5_89956")
        .expect("中间产物明细应含百花布");
    assert_eq!(zj.peifang_id, "tailoring_1285");
    assert_eq!(zj.danjia, 266_800);
    assert_eq!(zj.xiaojia, 266_800 * row.shuliang as i64);
    assert_eq!(zj.jingli, 8 * row.shuliang); // 百花布每件链路精力 20/2.5 = 8
    // 顶层单件成本 = 总成本 / 期望产出（HALF_UP，允许半个期望件的取整误差）
    let q = r.chuchan_qiwang;
    assert!(
        (r.danjia_chengben as f64 * q - r.zong_chengben as f64).abs() <= q / 2.0 + 0.5,
        "danjia_chengben {} × q {q} 应约等于 zong {}",
        r.danjia_chengben,
        r.zong_chengben,
    );
}

#[test]
fn bukede_material_on_real_data() {
    let catalog = repo_catalog();
    // furniture_202 驯兽密语笺：材料含水纹纸 5_3573（交易行无价、无 NPC 价、无合成配方）
    let r = craft::jisuan(&catalog, &request("furniture_202", BTreeMap::new()))
        .expect("驯兽密语笺应可计算（缺价按 0）");
    assert!(
        r.quejia.contains(&"5_3573".to_string()),
        "水纹纸应进缺价列表"
    );
    assert!(
        r.bukede.contains(&"5_3573".to_string()),
        "水纹纸既无价又无产出配方，应进不可获得列表"
    );
    // 不可获得是缺价的子集，且排序稳定
    assert!(r.bukede.iter().all(|id| r.quejia.contains(id)));
    let mut sorted = r.bukede.clone();
    sorted.sort();
    assert_eq!(r.bukede, sorted);
    // 数据侧核实：水纹纸无商店价、没有任何配方产出它
    let wupin = catalog
        .wupin
        .iter()
        .find(|w| w.id == "5_3573")
        .expect("数据里应有水纹纸");
    assert_eq!(wupin.shangdianjia, None);
    assert!(
        catalog
            .peifang
            .iter()
            .all(|p| p.chuchan.iter().all(|s| s.wupin_id != "5_3573"))
    );

    // 反向：可自制的中间产物（木板）缺价时不应被标为「不可获得」
    let r2 =
        craft::jisuan(&catalog, &request("furniture_58", BTreeMap::new())).expect("木板应可计算");
    assert!(r2.quejia.contains(&"5_35566".to_string()), "木料缺价");
    assert!(
        !r2.bukede.contains(&"5_35712".to_string()),
        "木板可自制，不算不可获得"
    );
}
