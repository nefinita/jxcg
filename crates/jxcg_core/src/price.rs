//! 材料取价的统一口径（NPC 商店价优先）。
//!
//! 优先级（用户 2026-11-12 明确要求）：
//! 1. **手填**：物品 id ∈ `shoudong` 且 `jiage` 有值 → 手填价（`shoudong`）；
//! 2. **商店价**：`wupin.shangdianjia > 0` → 商店价（`shangdian`），NPC 无限量售卖的材料不再用交易行价；
//! 3. **行情**：`jiage` 有值 → 行情价（`hangqing`）；
//! 4. **缺价**：以上都没有 → 0 并记入 `quejia`（`quejia`）。
//!
//! 不做「商店/行情取低」：只要 NPC 有售就以商店价为准（手填是用户显式覆盖，优先级更高）。
//! craft（含中间产物展开）、rank、convert、wuxingshi 的所有采购取价都必须走这里，
//! 避免不同模块口径漂移。

use std::collections::{BTreeMap, BTreeSet};

use crate::{JxcgError, Result};
use jxcg_api::{Catalog, Tong};

/// 取价核心（不做负数校验，供已保证价格合法的内部递归使用）。
///
/// 优先级：手填 > 商店价（> 0）> 行情（`jiage`）> 缺价。
/// 缺价时把物品 id 追加到 `quejia`（调用方负责排序去重）。
pub(crate) fn lookup(
    catalog: &Catalog,
    jiage: &BTreeMap<String, Tong>,
    shoudong: &BTreeSet<String>,
    wupin_id: &str,
    quejia: &mut Vec<String>,
) -> (Tong, &'static str) {
    // 1) 手填覆盖（仅当 jiage 里确有该物品价格时才算手填）
    if shoudong.contains(wupin_id)
        && let Some(price) = jiage.get(wupin_id)
    {
        return (*price, "shoudong");
    }
    // 2) NPC 商店价（> 0 才视为有售）
    if let Some(wupin) = catalog.wupin.iter().find(|w| w.id == wupin_id)
        && let Some(price) = wupin.shangdianjia
        && price > 0
    {
        return (price, "shangdian");
    }
    // 3) 行情价
    if let Some(price) = jiage.get(wupin_id) {
        return (*price, "hangqing");
    }
    // 4) 缺价
    quejia.push(wupin_id.to_string());
    (0, "quejia")
}

/// 取价（带负数校验）：被采用的单价为负时报中文错误。
///
/// 与 [`lookup`] 同一优先级；未被采用的 `jiage` 条目（如 NPC 有售时的交易行价）不参与校验。
pub(crate) fn qu_danjia(
    catalog: &Catalog,
    jiage: &BTreeMap<String, Tong>,
    shoudong: &BTreeSet<String>,
    wupin_id: &str,
    quejia: &mut Vec<String>,
) -> Result<(Tong, &'static str)> {
    let (price, laiyuan) = lookup(catalog, jiage, shoudong, wupin_id, quejia);
    if price < 0 {
        return Err(JxcgError::Money(format!(
            "单价不能为负：{wupin_id} = {price}（来源 {laiyuan}）"
        )));
    }
    Ok((price, laiyuan))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jxcg_api::{Banben, Pinlei, Wupin};

    fn catalog() -> Catalog {
        Catalog {
            banben: Banben::default(),
            wupin: vec![
                Wupin {
                    id: "npc".into(),
                    mingcheng: "NPC 商品".into(),
                    pinlei: Pinlei::Cailiao,
                    dui: None,
                    shangdianjia: Some(6_000),
                    laiyuan: None,
                },
                Wupin {
                    id: "normal".into(),
                    mingcheng: "普通材料".into(),
                    pinlei: Pinlei::Cailiao,
                    dui: None,
                    shangdianjia: None,
                    laiyuan: None,
                },
                Wupin {
                    id: "zero_shop".into(),
                    mingcheng: "商店价为 0".into(),
                    pinlei: Pinlei::Cailiao,
                    dui: None,
                    shangdianjia: Some(0),
                    laiyuan: None,
                },
            ],
            peifang: vec![],
            wuxingshi: vec![],
            wuxingshi_fangan: vec![],
            wuxingshi_hecheng: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        }
    }

    fn jia(id: &str, value: i64) -> BTreeMap<String, Tong> {
        BTreeMap::from([(id.to_string(), value)])
    }

    fn shoudong(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|id| (*id).to_string()).collect()
    }

    #[test]
    fn npc_shop_beats_market() {
        let cat = catalog();
        let mut quejia = Vec::new();
        // 即使行情更便宜（1 铜）也取商店价 6000
        let (price, source) =
            qu_danjia(&cat, &jia("npc", 1), &BTreeSet::new(), "npc", &mut quejia).unwrap();
        assert_eq!(price, 6_000);
        assert_eq!(source, "shangdian");
        assert!(quejia.is_empty());
        // 行情更贵（9000）同样取商店价
        let (price, source) = qu_danjia(
            &cat,
            &jia("npc", 9_000),
            &BTreeSet::new(),
            "npc",
            &mut quejia,
        )
        .unwrap();
        assert_eq!(price, 6_000);
        assert_eq!(source, "shangdian");
    }

    #[test]
    fn hand_filled_beats_npc_shop() {
        let cat = catalog();
        let mut quejia = Vec::new();
        let (price, source) = qu_danjia(
            &cat,
            &jia("npc", 250),
            &shoudong(&["npc"]),
            "npc",
            &mut quejia,
        )
        .unwrap();
        assert_eq!(price, 250);
        assert_eq!(source, "shoudong");
        // shoudong 命中但 jiage 没值 → 手填不生效，仍回落商店价
        let (price, source) = qu_danjia(
            &cat,
            &BTreeMap::new(),
            &shoudong(&["npc"]),
            "npc",
            &mut quejia,
        )
        .unwrap();
        assert_eq!(price, 6_000);
        assert_eq!(source, "shangdian");
    }

    #[test]
    fn market_priority_when_no_shop() {
        let cat = catalog();
        let mut quejia = Vec::new();
        let (price, source) = qu_danjia(
            &cat,
            &jia("normal", 123),
            &BTreeSet::new(),
            "normal",
            &mut quejia,
        )
        .unwrap();
        assert_eq!(price, 123);
        assert_eq!(source, "hangqing");
        // 商店价 0 不算有售 → 行情优先
        let (price, source) = qu_danjia(
            &cat,
            &jia("zero_shop", 77),
            &BTreeSet::new(),
            "zero_shop",
            &mut quejia,
        )
        .unwrap();
        assert_eq!(price, 77);
        assert_eq!(source, "hangqing");
        // 商店价 0 且无行情 → 缺价
        let (price, source) = qu_danjia(
            &cat,
            &BTreeMap::new(),
            &BTreeSet::new(),
            "zero_shop",
            &mut quejia,
        )
        .unwrap();
        assert_eq!(price, 0);
        assert_eq!(source, "quejia");
        assert_eq!(quejia, vec!["zero_shop".to_string()]);
    }

    #[test]
    fn negative_price_validation() {
        let cat = catalog();
        let mut quejia = Vec::new();
        // 被采用的手填价/行情价为负 → 中文错误
        let err = qu_danjia(
            &cat,
            &jia("normal", -1),
            &BTreeSet::new(),
            "normal",
            &mut quejia,
        )
        .unwrap_err();
        assert!(err.to_string().contains("单价不能为负"));
        // NPC 有售时交易行负价不被采用，不报错
        let (price, source) =
            qu_danjia(&cat, &jia("npc", -5), &BTreeSet::new(), "npc", &mut quejia).unwrap();
        assert_eq!(price, 6_000);
        assert_eq!(source, "shangdian");
    }
}
