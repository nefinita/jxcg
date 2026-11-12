//! 真实随包数据（data/wuxingshi.toml）上的五行石面板合成金标准测试。
//!
//! 合成次数/材料颗数/精力/加工费与行情无关，可直接钉死 2026-11 面板口径；
//! 行情相关部分（副产物折价、利润）在单测里用合成价格断言。

use std::collections::BTreeMap;

use jxcg_api::{Catalog, Quzheng, Shouxufei, WuxingshiLianRequest};
use jxcg_core::{store, wuxingshi};

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

fn request() -> WuxingshiLianRequest {
    WuxingshiLianRequest {
        fangan_id: "mianban".into(),
        mubiao_wupin_id: wuxingshi::LIUJI_WUPIN_ID.into(),
        shuliang: 1,
        jingli_danjia: 0,
        shouxufei: cfg(),
        // 一级石按 1 金/颗，便于用材料成本反推精确期望数量
        jiage: BTreeMap::from([("5_24423".to_string(), 10_000)]),
        baoji_bp: None,
    }
}

#[test]
fn panel_chain_golden_on_repo_data() {
    let catalog = repo_catalog();
    assert!(
        catalog.wuxingshi_fangan.iter().any(|f| f.id == "mianban"),
        "data/wuxingshi.toml 应包含 mianban 方案"
    );
    let r = wuxingshi::jisuan_lian(&catalog, &request()).expect("面板方案应可计算");
    assert_eq!(r.fangan_mingcheng, "面板配方（游戏内一键合成）");
    assert_eq!(r.mubiao_mingcheng, "五行石（六级）");
    assert_eq!(r.cailiao.len(), 1);
    assert_eq!(r.cailiao[0].wupin_id, "5_24423");
    assert_eq!(r.cailiao[0].shuliang, 593);
    assert_eq!(r.cailiao_chengben, 5_929_528); // 592.9528 颗 × 1 金
    assert_eq!(r.jingli, 581);
    assert_eq!(r.jiagongfei, 679_704);
    assert_eq!(
        r.buzhou
            .iter()
            .map(|b| b.hecheng_id.as_str())
            .collect::<Vec<_>>(),
        vec!["mianban_3", "mianban_4", "mianban_5", "mianban_6"]
    );
    assert_eq!(
        r.zong_chengben,
        r.cailiao_chengben + r.jingli_chengben + r.jiagongfei - r.fuchanpin_shouru
    );
}

#[test]
fn panel_chain_baoji_on_repo_data() {
    let catalog = repo_catalog();
    let mut req = request();
    req.baoji_bp = Some(5); // 合 6 出 8 级 0.05%
    req.jiage.insert("5_24430".to_string(), 255_000_000); // 八级 25500 金
    let r = wuxingshi::jisuan_lian(&catalog, &req).expect("面板方案应可计算");
    let eight = r
        .fuchanpin
        .iter()
        .find(|f| f.wupin_id == wuxingshi::BAOJI_WUPIN_ID)
        .expect("覆盖 0.05% 后应出现八级副产物");
    assert!((eight.qiwang - 0.000_502_260_17).abs() < 1e-9);
    assert_eq!(eight.shouru, 128_076); // 约 12.8 金/颗
    assert_eq!(
        r.zong_chengben,
        r.cailiao_chengben + r.jingli_chengben + r.jiagongfei - r.fuchanpin_shouru
    );
}
