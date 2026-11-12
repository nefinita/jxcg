//! 真实随包数据（data/）上的中间产物冒烟测试。
//!
//! 与合成目录的单测互补：确认全量数据（~1036 配方）能派生中间产物索引、
//! 递归展开不 panic，且 `zong_chengben` 严格等于各项之和（数据漂移时先在这里报警）。

use std::collections::{BTreeMap, BTreeSet};

use jxcg_api::{Catalog, JisuanRequest, PaihangRequest, PaihangZhibiao, Shouxufei};
use jxcg_core::{craft, rank, store};

fn repo_catalog() -> Catalog {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    store::load_catalog(&dir).expect("data/ 应能加载")
}

#[test]
fn middle_index_on_repo_data() {
    let catalog = repo_catalog();
    let empty_price = BTreeMap::new();
    let empty_shoudong = BTreeSet::new();
    let calculator = craft::Calculator::new(&catalog, &empty_price, &empty_shoudong, 0);
    let index = calculator.index();
    // 当前数据口径：32 个中间产物（产物 ∩ 材料）；数据更新后仍应 > 0
    assert!(
        !index.is_empty(),
        "真实数据里应存在「产物 ∩ 材料」的中间产物"
    );
    // 每个中间产物都必须有产出配方（否则无法作为成本来源）
    for peifang in &catalog.peifang {
        for slot in &peifang.chuchan {
            if index.is_zhongjian(&slot.wupin_id) {
                assert!(
                    index.producer(&slot.wupin_id).is_some(),
                    "中间产物 {} 缺少产出配方",
                    slot.wupin_id
                );
            }
        }
    }
    // 只产中间产物的配方应存在（排行默认排除它们）
    let zhongjian_recipes = catalog
        .peifang
        .iter()
        .filter(|peifang| index.is_zhongjian_peifang(peifang))
        .count();
    assert!(zhongjian_recipes > 0, "应存在只产中间产物的配方");
}

#[test]
fn expand_repo_data_recipes_smoke() {
    let catalog = repo_catalog();
    let empty_price = BTreeMap::new();
    let empty_shoudong = BTreeSet::new();
    let mut calculator = craft::Calculator::new(&catalog, &empty_price, &empty_shoudong, 0);

    // 找出「材料里含中间产物」的真实配方（最多 20 个，覆盖各技艺与多级链路）
    let ids: Vec<String> = catalog
        .peifang
        .iter()
        .filter(|peifang| {
            peifang
                .cailiao
                .iter()
                .any(|item| calculator.index().is_zhongjian(&item.wupin_id))
        })
        .map(|peifang| peifang.id.clone())
        .take(20)
        .collect();
    assert!(!ids.is_empty(), "真实数据应存在用到中间产物的配方");

    for id in ids {
        let req = JisuanRequest {
            peifang_id: id.clone(),
            cishu: 1,
            guadan_cishu: 1,
            jingli_danjia: 0,
            shouxufei: Shouxufei::default(),
            jiage: BTreeMap::new(),
            jiyi_dengji: 0,
            zhongjian_zizhi: true,
            shoudong: BTreeSet::new(),
        };
        let r = calculator.jisuan(&req).expect("真实配方应能计算");
        assert_eq!(
            r.zong_chengben,
            r.chengben_cailiao + r.chengben_jingli + r.zhizuofei + r.baoguanfei,
            "{} 的总成本必须等于各项之和",
            id
        );
        assert!(
            !r.zhongjian.is_empty(),
            "{} 含中间产物材料，应产生自制明细",
            id
        );
        assert!(r.chengben_cailiao >= 0);
    }
}

#[test]
fn rank_repo_data_smoke() {
    // 全量排行冒烟：一次请求内索引/单位成本 memo 复用，1036 配方不 panic、结果稳定。
    let catalog = repo_catalog();
    let req = PaihangRequest {
        jiyi: None,
        jingli_danjia: 0,
        shouxufei: Shouxufei::default(),
        jiage: BTreeMap::new(),
        zhibiao: PaihangZhibiao::Lirun,
        jiyi_dengji: 0,
        limit: None,
        baohan_quejia: true,
        zhongjian_zizhi: true,
        baohan_zhongjian: true,
        shoudong: BTreeSet::new(),
    };
    let rows = rank::paihang(&catalog, &req);
    assert!(
        rows.len() > 500,
        "全量数据排行应覆盖多数配方（实际 {}）",
        rows.len()
    );
    // 只产中间产物的配方被正确标记
    assert!(rows.iter().any(|row| row.zhongjian), "应存在中间产物配方行");
    // 排序确定：缺价行在末尾，其余按净利润降序、同值 id 升序
    let mut saw_quejia = false;
    for pair in rows.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.quejia {
            saw_quejia = true;
        }
        assert!(!(saw_quejia && !b.quejia), "缺价行之后不应再有有效行");
        if !a.quejia && !b.quejia {
            assert!(
                a.lirun > b.lirun || (a.lirun == b.lirun && a.peifang_id < b.peifang_id),
                "排序不稳定：{} vs {}",
                a.peifang_id,
                b.peifang_id
            );
        }
    }
}
