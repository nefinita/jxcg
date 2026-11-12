// ============================================================================
// 赛季（资料片）过滤：只保留本赛季用得上的配方
//
// 背景：JX3BOX 配方详情里的 `nDlc` 是资料片版本（当前正式服最高 9 = 苍生铸世/50 级（2026-10-29 等级回退）），
// 旧资料片配方（5/6/7/8 或 null）大多已被淘汰（旧绝世装备/旧宴席/饺子皮馅料等），
// 用户要求计算器里不出现它们。
//
// 保留规则（默认 `--season auto`）：
// a. `nDlc == current_dlc`（本资料片配方）；
// b. 梓匠（`furniture`）全部保留——家具配方没有 nDlc，且本赛季晋秩仍要木板/卯榫等；
// c. **闭包补齐**：任何配方的产物若被「已保留配方」当材料用，则该配方也保留
//    （保住本赛季仍在用的旧中间材料配方：各类锭/布/药引/馅料/酱料…）。
//
// 其余配方剔除；物品表随后只保留被保留配方引用的物品。
// ============================================================================

use std::collections::BTreeSet;

use crate::api::RecipeDetail;
use crate::util::item_sort_key;

/// 资料片版本 → 中文名（当前只确认 9 = 苍生铸世；新资料片更新此表或让 note 显示 nDlc）。
pub fn season_mingcheng(dlc: i64) -> Option<&'static str> {
    const NAMES: &[(i64, &str)] = &[(9, "苍生铸世")];
    NAMES
        .iter()
        .find(|(version, _)| *version == dlc)
        .map(|(_, name)| *name)
}

/// 过滤结果。
#[derive(Debug, Clone, Default)]
pub struct SeasonFilter {
    /// 保留掩码，与输入 details 等长
    pub kept: Vec<bool>,
    /// 闭包补进来的配方下标（按加入顺序）
    pub closure_added: Vec<usize>,
    /// 因「旧家园家具成品（梓匠产物 10_）」被剔除的配方下标
    pub old_home_removed: Vec<usize>,
    /// 保留配方里派生的中间产物 id（产物 ∩ 材料，按物品 id 数值排序去重）
    pub intermediate: Vec<String>,
}

impl SeasonFilter {
    pub fn kept_count(&self) -> usize {
        self.kept.iter().filter(|kept| **kept).count()
    }

    pub fn removed_count(&self) -> usize {
        self.kept.iter().filter(|kept| !**kept).count()
    }
}

/// 当前资料片：所有配方 `nDlc` 的最大值（忽略 null）。
pub fn detect_current_dlc(details: &[RecipeDetail]) -> Option<i64> {
    details.iter().filter_map(|detail| detail.n_dlc).max()
}

fn item_id(item_type: u32, index: u32) -> String {
    crate::api::item_id(item_type, index)
}

/// 是否旧家园家具成品配方：梓匠且任一产物是 `10_` 家居物品。
///
/// 这类成品依赖绑定/不可获得的阅读材料（如徽墨 5_3843）：NPC 无售卖价、无合成配方、
/// 交易行也没有稳定挂单，按用户口径属于「不可获得材料」，本赛季不参与交易行计算。
fn is_old_home_furniture(detail: &RecipeDetail) -> bool {
    detail.tab == "furniture"
        && detail
            .chuchan
            .iter()
            .any(|(item_type, _, _, _, _)| *item_type == 10)
}

/// 某个配方是否属于「基础保留」集合（规则 a + b）。
fn is_base_kept(detail: &RecipeDetail, current: Option<i64>) -> bool {
    match current {
        None => true,
        Some(current) => detail.n_dlc == Some(current) || detail.tab == "furniture",
    }
}

/// 按赛季过滤配方，并做旧中间材料闭包；返回保留掩码与中间产物清单。
pub fn filter_by_season(details: &[RecipeDetail], current: Option<i64>) -> SeasonFilter {
    let mut filter = SeasonFilter {
        kept: details
            .iter()
            .map(|detail| is_base_kept(detail, current))
            .collect(),
        ..SeasonFilter::default()
    };
    if current.is_none() {
        // --all：不剔除，但仍计算中间产物清单（README/排障用）
        filter.intermediate = intermediate_ids(details, &filter.kept);
        return filter;
    }

    // 闭包：迭代加入「产物是保留配方材料」的配方，直到不再新增。
    loop {
        let mut needed_materials: BTreeSet<String> = BTreeSet::new();
        for (detail, kept) in details.iter().zip(filter.kept.iter()) {
            if !kept {
                continue;
            }
            for (item_type, index, _) in &detail.cailiao {
                needed_materials.insert(item_id(*item_type, *index));
            }
        }
        let mut added_any = false;
        for (index, detail) in details.iter().enumerate() {
            if filter.kept[index] {
                continue;
            }
            let produces_needed = detail
                .chuchan
                .iter()
                .any(|(item_type, item_index, _, _, _)| {
                    needed_materials.contains(&item_id(*item_type, *item_index))
                });
            if produces_needed {
                filter.kept[index] = true;
                filter.closure_added.push(index);
                added_any = true;
            }
        }
        if !added_any {
            break;
        }
    }

    // 规则 d：剔除旧家园家具成品（梓匠产物 10_）——依赖绑定/不可获得材料，没有可靠的交易行价格来源。
    for (index, (detail, kept)) in details.iter().zip(filter.kept.iter_mut()).enumerate() {
        if *kept && is_old_home_furniture(detail) {
            *kept = false;
            filter.old_home_removed.push(index);
        }
    }

    filter.intermediate = intermediate_ids(details, &filter.kept);
    filter
}

/// 保留配方里派生的中间产物：既是某保留配方的产物，又是某保留配方的材料。
pub fn intermediate_ids(details: &[RecipeDetail], kept: &[bool]) -> Vec<String> {
    let mut materials: BTreeSet<String> = BTreeSet::new();
    let mut products: BTreeSet<String> = BTreeSet::new();
    for (detail, is_kept) in details.iter().zip(kept.iter()) {
        if !is_kept {
            continue;
        }
        for (item_type, index, _) in &detail.cailiao {
            materials.insert(item_id(*item_type, *index));
        }
        for (item_type, index, _, _, _) in &detail.chuchan {
            products.insert(item_id(*item_type, *index));
        }
    }
    let mut middle: Vec<String> = materials.intersection(&products).cloned().collect();
    middle.sort_by_key(|id| item_sort_key(id));
    middle
}

/// 按技艺统计保留/剔除条数：(tab, 条数)，顺序与 `api::JIYI_TABS` 一致。
pub fn count_by_tab(
    details: &[RecipeDetail],
    kept: &[bool],
    want_kept: bool,
) -> Vec<(&'static str, usize)> {
    crate::api::JIYI_TABS
        .iter()
        .map(|(tab, _)| {
            let count = details
                .iter()
                .zip(kept.iter())
                .filter(|(detail, is_kept)| detail.tab == *tab && **is_kept == want_kept)
                .count();
            (*tab, count)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小配方（材料/产物都是 5_xxx 物品）。
    fn recipe(
        tab: &str,
        game_id: i64,
        n_dlc: Option<i64>,
        cailiao: &[(u32, u32)],
        chuchan: &[(u32, u32)],
    ) -> RecipeDetail {
        RecipeDetail {
            tab: tab.to_string(),
            game_id,
            name: format!("{tab}_{game_id}"),
            require_level: 50,
            cost_vigor: 10,
            n_dlc,
            cailiao: cailiao.iter().map(|(t, i)| (*t, *i, 1)).collect(),
            chuchan: chuchan
                .iter()
                .map(|(t, i)| (*t, *i, 1, 1, 1_048_576))
                .collect(),
        }
    }

    #[test]
    fn detect_max_ndlc() {
        let details = vec![
            recipe("cooking", 1, None, &[], &[(5, 100)]),
            recipe("cooking", 2, Some(8), &[], &[(5, 101)]),
            recipe("tailoring", 3, Some(9), &[], &[(5, 102)]),
            recipe("founding", 4, Some(9), &[], &[(5, 103)]),
        ];
        assert_eq!(detect_current_dlc(&details), Some(9));
        assert_eq!(detect_current_dlc(&[]), None);
    }

    #[test]
    fn filter_keeps_current_furniture_and_closure() {
        let details = vec![
            // A：本赛季配方，用中间材料 5_100
            recipe("cooking", 1, Some(9), &[(5, 100)], &[(5, 900)]),
            // B：旧配方，但产物 5_100 是 A 的材料 → 闭包保留（第 1 轮）
            recipe("cooking", 2, Some(1), &[(5, 200)], &[(5, 100)]),
            // C：更旧的配方，产物 5_200 是 B 的材料 → 闭包保留（第 2 轮，传递闭包）
            recipe("cooking", 3, None, &[(5, 300)], &[(5, 200)]),
            // D：无关旧配方 → 剔除
            recipe("cooking", 4, Some(8), &[(5, 400)], &[(5, 999)]),
            // E：梓匠无 nDlc，但产物是 10_ 旧家园家具 → 规则 d 剔除
            recipe("furniture", 5, None, &[(5, 500)], &[(10, 7)]),
        ];
        let filter = filter_by_season(&details, Some(9));
        assert_eq!(filter.kept, vec![true, true, true, false, false]);
        assert_eq!(filter.kept_count(), 3);
        assert_eq!(filter.removed_count(), 2);
        assert_eq!(filter.old_home_removed, vec![4]);
        assert_eq!(filter.closure_added.len(), 2);
        // 中间产物：5_100（B 产 / A 用）、5_200（C 产 / B 用）
        assert_eq!(
            filter.intermediate,
            vec!["5_100".to_string(), "5_200".to_string()]
        );
    }

    #[test]
    fn filter_removes_old_dumpling_skin_recipe() {
        // 用录制 JSON 走「解析 + 过滤」全链路：饺子皮（nDlc=null、无保留配方用它）应被剔除。
        let cooking_437 = load_fixture("detail_cooking_437.json", "cooking", 437, "云芝竹沥羹", 50);
        let cooking_120 = load_fixture("detail_cooking_120.json", "cooking", 120, "饺子皮", 1);
        let furniture_1 = load_fixture("detail_furniture_1.json", "furniture", 1, "柳叶水甘草", 9);
        let details = vec![cooking_437, cooking_120, furniture_1];
        assert_eq!(detect_current_dlc(&details), Some(9));
        let filter = filter_by_season(&details, Some(9));
        // 饺子皮（旧赛季）与家具_1（产物 10_7 旧家园家具）都应剔除
        assert_eq!(filter.kept, vec![true, false, false]);
        assert_eq!(filter.old_home_removed, vec![2]);
    }

    fn load_fixture(
        file: &str,
        tab: &str,
        game_id: i64,
        fallback_name: &str,
        fallback_level: i64,
    ) -> RecipeDetail {
        let text = match file {
            "detail_cooking_437.json" => include_str!("../tests/fixtures/detail_cooking_437.json"),
            "detail_cooking_120.json" => include_str!("../tests/fixtures/detail_cooking_120.json"),
            "detail_furniture_1.json" => include_str!("../tests/fixtures/detail_furniture_1.json"),
            _ => panic!("未知 fixture：{file}"),
        };
        let value: serde_json::Value = serde_json::from_str(text).expect("fixture 解析失败");
        crate::api::parse_recipe_detail(tab, game_id, fallback_name, fallback_level, &value)
            .expect("配方详情解析失败")
    }

    #[test]
    fn all_mode_keeps_everything() {
        let details = vec![
            recipe("cooking", 1, Some(9), &[(5, 1)], &[(5, 2)]),
            recipe("cooking", 2, Some(5), &[(5, 2)], &[(5, 3)]),
            recipe("founding", 3, None, &[(5, 9)], &[(5, 1)]),
        ];
        let mut details = details;
        details.push(recipe("furniture", 4, None, &[], &[(10, 7)]));
        let filter = filter_by_season(&details, None);
        // --all：完全不剔除（旧家园家具也保留），仅计算中间产物
        assert_eq!(filter.kept, vec![true, true, true, true]);
        assert!(filter.closure_added.is_empty());
        assert!(filter.old_home_removed.is_empty());
        assert_eq!(
            filter.intermediate,
            vec!["5_1".to_string(), "5_2".to_string()]
        );
    }

    #[test]
    fn removes_old_home_furniture_products_10() {
        let details = vec![
            recipe("cooking", 1, Some(9), &[], &[(5, 900)]),
            // 5_ 产物（木板等可交易材料）→ 保留
            recipe("furniture", 2, None, &[(5, 501)], &[(5, 35712)]),
            // 10_ 产物（旧家园家具成品）→ 剔除，且其绑定材料不再进物品表
            recipe("furniture", 3, None, &[(5, 3843)], &[(10, 1772)]),
            recipe("furniture", 4, None, &[(5, 3843)], &[(10, 1773)]),
        ];
        let filter = filter_by_season(&details, Some(9));
        assert_eq!(filter.kept, vec![true, true, false, false]);
        assert_eq!(filter.old_home_removed, vec![2, 3]);
        // 徽墨（5_3843）只被剔除的 10_ 配方使用 → 不再是中间产物/保留引用
        assert!(!filter.intermediate.contains(&"5_3843".to_string()));
    }

    #[test]
    fn count_by_tab_splits_kept_and_removed() {
        let details = vec![
            recipe("cooking", 1, Some(9), &[], &[(5, 1)]),
            recipe("cooking", 2, Some(8), &[], &[(5, 2)]),
            recipe("furniture", 3, None, &[], &[(5, 7)]),
        ];
        let filter = filter_by_season(&details, Some(9));
        let kept = count_by_tab(&details, &filter.kept, true);
        let removed = count_by_tab(&details, &filter.kept, false);
        assert_eq!(
            kept.iter()
                .find(|(tab, _)| *tab == "cooking")
                .map(|(_, n)| *n),
            Some(1)
        );
        assert_eq!(
            kept.iter()
                .find(|(tab, _)| *tab == "furniture")
                .map(|(_, n)| *n),
            Some(1)
        );
        assert_eq!(
            removed
                .iter()
                .find(|(tab, _)| *tab == "cooking")
                .map(|(_, n)| *n),
            Some(1)
        );
    }
}
