//! JX3BOX 静态数据接口（2026-11-12 实测，HTTPS 免 token）。
//!
//! - 配方目录：`node.jx3box.com/manufactures?client=std&type={tab}&mode=simple`
//! - 配方详情：`node.jx3box.com/manufacture/{tab}/{id}?client=std`
//! - 物品：`node.jx3box.com/resource/std/item_merged.{类型}_{索引}`（不存在时返回 `[]`）
//! - NPC 商店价：`node.jx3box.com/craft/price?client=std`
//! - 服务器：`raw.githubusercontent.com/JX3BOX/jx3box-data/master/data/server/*.json`
//!
//! 接口字段里数字槽位可能是 number / null / false（游戏 Lua 数据直出），
//! 统一用 `serde_json::Value` 做宽松取值，避免上游小改字段就整批解析失败。

use std::collections::BTreeMap;

use jxcg_api::{Fuwuqi, Jiyi};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::http::HttpClient;
use crate::util::pinyin_id;

pub const NODE_BASE: &str = "https://node.jx3box.com";
pub const RAW_BASE: &str = "https://raw.githubusercontent.com/JX3BOX/jx3box-data/master";

/// 5 技艺：JX3BOX 目录 type → 本地 `jiyi` 枚举。
pub const JIYI_TABS: [(&str, Jiyi); 5] = [
    ("cooking", Jiyi::Pengren),
    ("medicine", Jiyi::Zhiyao),
    ("tailoring", Jiyi::Fengren),
    ("founding", Jiyi::Zhuzhao),
    ("furniture", Jiyi::Zijiang),
];

/// 表格 tab → `jiyi`。
pub fn jiyi_of_tab(tab: &str) -> Option<Jiyi> {
    JIYI_TABS
        .iter()
        .find(|(name, _)| *name == tab)
        .map(|(_, jiyi)| *jiyi)
}

/// 物品 id：`"{类型}_{索引}"`，材料基本都是类型 5，家具成品是类型 10。
pub fn item_id(item_type: u32, index: u32) -> String {
    format!("{item_type}_{index}")
}

// ============================================================================
// 配方目录
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    #[serde(rename = "ID", default)]
    pub id: i64,
    #[serde(rename = "Name", default)]
    pub name: String,
    /// 配方等级；0 表示未开放/占位数据
    #[serde(rename = "nLevel", default)]
    pub n_level: i64,
}

impl CatalogEntry {
    /// 过滤占位数据（任务书口径：ID>0 && nLevel>0 && Name 非空）。
    pub fn is_valid(&self) -> bool {
        self.id > 0 && self.n_level > 0 && !self.name.trim().is_empty()
    }
}

pub fn catalog_url(tab: &str) -> String {
    format!("{NODE_BASE}/manufactures?client=std&type={tab}&mode=simple")
}

pub fn detail_url(tab: &str, id: i64) -> String {
    format!("{NODE_BASE}/manufacture/{tab}/{id}?client=std")
}

pub fn item_url(item_id: &str) -> String {
    format!("{NODE_BASE}/resource/std/item_merged.{item_id}")
}

pub fn craft_price_url() -> String {
    format!("{NODE_BASE}/craft/price?client=std")
}

pub fn server_map_url() -> String {
    format!("{RAW_BASE}/data/server/server_map.json")
}

pub fn server_zone_url() -> String {
    format!("{RAW_BASE}/data/server/server_zone.json")
}

/// 把响应兜底成数组：兼容 `[...]` 与 `{ code, data: [...] }` 两种包法。
fn into_list(value: Value) -> Value {
    match value {
        Value::Array(_) => value,
        Value::Object(mut map) => map.remove("data").unwrap_or(Value::Array(Vec::new())),
        _ => Value::Array(Vec::new()),
    }
}

pub async fn fetch_catalog(client: &HttpClient, tab: &str) -> Result<Vec<CatalogEntry>> {
    let value = client.get_json(&catalog_url(tab)).await?;
    let list: Vec<CatalogEntry> = serde_json::from_value(into_list(value))
        .map_err(|err| Error::Parse(format!("配方目录解析失败（{tab}）：{err}")))?;
    Ok(list.into_iter().filter(CatalogEntry::is_valid).collect())
}

// ============================================================================
// 配方详情
// ============================================================================

/// 详情解析后的配方（槽位已按任务书口径过滤过）。
#[derive(Debug, Clone)]
pub struct RecipeDetail {
    pub tab: String,
    pub game_id: i64,
    pub name: String,
    pub require_level: u32,
    pub cost_vigor: u32,
    /// 资料片版本（JX3BOX `nDlc`）：当前正式服最高 9（苍生铸世），旧配方为 5/6/7/8 或 null
    pub n_dlc: Option<i64>,
    /// 材料槽：(物品类型, 物品索引, 数量)，只保留数量 > 0 的槽
    pub cailiao: Vec<(u32, u32, u32)>,
    /// 产物槽：(物品类型, 物品索引, min, max, 概率权重)，只保留权重 > 0 的槽
    pub chuchan: Vec<(u32, u32, u32, u32, u32)>,
}

pub async fn fetch_recipe_detail(
    client: &HttpClient,
    tab: &str,
    entry: &CatalogEntry,
) -> Result<RecipeDetail> {
    let value = client.get_json(&detail_url(tab, entry.id)).await?;
    parse_recipe_detail(tab, entry.id, &entry.name, entry.n_level, &value)
}

/// 解析配方详情 JSON（纯函数，便于用录制 JSON 做离线测试）。
///
/// `nDlc` 是资料片版本：当前正式服最高 9（苍生铸世/50 级），
/// 旧资料片配方是 5/6/7/8 或 null（见 season 模块的过滤规则）。
pub fn parse_recipe_detail(
    tab: &str,
    game_id: i64,
    fallback_name: &str,
    fallback_level: i64,
    value: &Value,
) -> Result<RecipeDetail> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Parse(format!("配方详情不是对象（{tab}#{game_id}）")))?;

    let name = str_field(obj, "Name")
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback_name.to_string());
    let require_level = positive_i64(obj.get("RequireLevel"))
        .or_else(|| positive_i64(obj.get("nLevel")))
        .unwrap_or(fallback_level.max(0)) as u32;
    let cost_vigor = non_negative_i64(obj.get("CostVigor")).unwrap_or(0) as u32;
    let n_dlc = non_negative_i64(obj.get("nDlc"));

    let mut cailiao = Vec::new();
    for index in 1..=8 {
        let item_type = positive_i64(obj.get(&format!("RequireItemType{index}")));
        let item_index = positive_i64(obj.get(&format!("RequireItemIndex{index}")));
        let count = positive_i64(obj.get(&format!("RequireItemCount{index}")));
        if let (Some(item_type), Some(item_index), Some(count)) = (item_type, item_index, count) {
            cailiao.push((
                item_type as u32,
                item_index as u32,
                count.min(u32::MAX as i64) as u32,
            ));
        }
    }

    let mut chuchan = Vec::new();
    for index in 1..=8 {
        let item_type = positive_i64(obj.get(&format!("CreateItemType{index}")));
        let item_index = positive_i64(obj.get(&format!("CreateItemIndex{index}")));
        let probability = positive_i64(obj.get(&format!("CreateItemProbability{index}")));
        let (Some(item_type), Some(item_index), Some(probability)) =
            (item_type, item_index, probability)
        else {
            continue;
        };
        let min = non_negative_i64(obj.get(&format!("CreateItemMin{index}")))
            .unwrap_or(1)
            .max(1);
        let max = non_negative_i64(obj.get(&format!("CreateItemMax{index}")))
            .unwrap_or(min)
            .max(min);
        chuchan.push((
            item_type as u32,
            item_index as u32,
            min.min(u32::MAX as i64) as u32,
            max.min(u32::MAX as i64) as u32,
            probability.min(u32::MAX as i64) as u32,
        ));
    }

    Ok(RecipeDetail {
        tab: tab.to_string(),
        game_id,
        name,
        require_level,
        cost_vigor,
        n_dlc,
        cailiao,
        chuchan,
    })
}

// ============================================================================
// 物品
// ============================================================================

#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub name: String,
    /// JX3BOX 的粗分类标签：材料 / 食物 / 五行石 / 景观 …
    pub type_label: String,
}

pub async fn fetch_item(client: &HttpClient, item_id: &str) -> Result<Option<ItemInfo>> {
    let value = client.get_json(&item_url(item_id)).await?;
    let Some(obj) = item_object(value) else {
        return Ok(None);
    };
    let Some(name) = str_field(&obj, "Name").filter(|name| !name.trim().is_empty()) else {
        return Ok(None);
    };
    let type_label = str_field(&obj, "TypeLabel").unwrap_or_default();
    Ok(Some(ItemInfo { name, type_label }))
}

/// 物品接口对不存在的 id 返回 `[]`；部分接口可能包一层 `{ data: {...} }`。
fn item_object(value: Value) -> Option<Map<String, Value>> {
    match value {
        Value::Object(map) => {
            if map.contains_key("Name") {
                Some(map)
            } else if let Some(Value::Object(inner)) = map.get("data").cloned() {
                Some(inner)
            } else {
                None
            }
        }
        Value::Array(items) => items.into_iter().find_map(|item| match item {
            Value::Object(map) => Some(map),
            _ => None,
        }),
        _ => None,
    }
}

// ============================================================================
// NPC 商店价
// ============================================================================

#[derive(Debug, Deserialize)]
struct CraftPriceRow {
    #[serde(rename = "ItemIndex", default)]
    item_index: i64,
    #[serde(rename = "Price", default)]
    price: i64,
    #[serde(rename = "IsValid", default)]
    is_valid: bool,
}

/// NPC 售卖价（铜）：物品索引 → 价格。只有生活技艺材料商出售的少数材料有值。
pub async fn fetch_craft_prices(client: &HttpClient) -> Result<BTreeMap<u32, i64>> {
    let value = client.get_json(&craft_price_url()).await?;
    let rows: Vec<CraftPriceRow> = serde_json::from_value(into_list(value))
        .map_err(|err| Error::Parse(format!("商店价解析失败：{err}")))?;
    let mut map = BTreeMap::new();
    for row in rows {
        if row.is_valid && row.item_index > 0 && row.price > 0 {
            map.insert(row.item_index as u32, row.price);
        }
    }
    Ok(map)
}

// ============================================================================
// 服务器
// ============================================================================

#[derive(Debug, Default, Deserialize)]
struct ServerMapEntry {
    #[serde(default)]
    client: Option<String>,
}

/// 服务器列表：`server_map.json`（含正式服/缘起/国际服）+ `server_zone.json`（大区）。
///
/// - `id`：pinyin（本地文件名/设置项用）；
/// - `mingcheng`：中文名（JX3BOX 行情接口的 server 参数就是这个）；
/// - `daju`：`server_zone.json` 里的大区；缘起/国际服按 client 标注，未知留空。
pub async fn fetch_servers(client: &HttpClient) -> Result<Vec<Fuwuqi>> {
    let map_value = client.get_json(&server_map_url()).await?;
    let map: BTreeMap<String, ServerMapEntry> = serde_json::from_value(map_value)
        .map_err(|err| Error::Parse(format!("服务器列表解析失败：{err}")))?;
    let zone_value = client.get_json(&server_zone_url()).await?;
    let zones: BTreeMap<String, String> = serde_json::from_value(zone_value)
        .map_err(|err| Error::Parse(format!("服务器大区解析失败：{err}")))?;

    let mut out = Vec::with_capacity(map.len());
    for (mingcheng, entry) in map {
        let id = pinyin_id(&mingcheng).map(str::to_string).unwrap_or_else(|| {
            eprintln!("  [提示] 服务器「{mingcheng}」未收录 pinyin 表，id 暂用中文名（请补 util::pinyin_id）");
            mingcheng.clone()
        });
        let daju = zones
            .get(&mingcheng)
            .cloned()
            .or_else(|| match entry.client.as_deref() {
                Some("origin") => Some("缘起大区".to_string()),
                Some("std") if is_international(&mingcheng) => Some("国际服".to_string()),
                _ => None,
            });
        out.push(Fuwuqi {
            id,
            mingcheng: mingcheng.clone(),
            daju,
        });
    }
    // 已知大区在前（按名称），未知大区垫底；同区按服务器名。
    out.sort_by(|a, b| {
        a.daju
            .is_none()
            .cmp(&b.daju.is_none())
            .then_with(|| {
                a.daju
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.daju.as_deref().unwrap_or(""))
            })
            .then_with(|| a.mingcheng.cmp(&b.mingcheng))
    });
    Ok(out)
}

/// 国际服常见繁体名（server_map.json 里 client 也标 std，只能按名字粗判大区）。
fn is_international(mingcheng: &str) -> bool {
    const INTL: &[&str] = &["共結來緣", "傲血戰意", "巔峰再起", "江海雲夢"];
    INTL.contains(&mingcheng)
}

// ============================================================================
// JSON 宽松取值
// ============================================================================

fn positive_i64(value: Option<&Value>) -> Option<i64> {
    non_negative_i64(value).filter(|value| *value > 0)
}

fn non_negative_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().map(|v| v as i64)),
        // 少量槽位是 bool（false 表示空槽），true 按 1 处理
        Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(1),
        _ => None,
    }
}

fn str_field(obj: &Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail_fixture() -> Map<String, Value> {
        let text = include_str!("../tests/fixtures/detail_cooking_437.json");
        let value: Value = serde_json::from_str(text).expect("fixture 解析失败");
        value.as_object().cloned().expect("fixture 不是对象")
    }

    #[test]
    fn parse_slots_from_recorded_detail() {
        let obj = detail_fixture();
        // 直接复用 fetch 里的解析逻辑：用 http 层无网络的方式不方便，这里手动走一遍字段提取。
        let mut cailiao = Vec::new();
        for index in 1..=8 {
            let item_type = positive_i64(obj.get(&format!("RequireItemType{index}")));
            let item_index = positive_i64(obj.get(&format!("RequireItemIndex{index}")));
            let count = positive_i64(obj.get(&format!("RequireItemCount{index}")));
            if let (Some(item_type), Some(item_index), Some(count)) = (item_type, item_index, count)
            {
                cailiao.push((item_type as u32, item_index as u32, count as u32));
            }
        }
        assert_eq!(
            cailiao,
            vec![
                (5, 30_853, 1),
                (5, 30_845, 3),
                (5, 89_970, 3),
                (5, 89_167, 1),
                (5, 31_984, 10)
            ]
        );

        let mut chuchan = Vec::new();
        for index in 1..=8 {
            let item_type = positive_i64(obj.get(&format!("CreateItemType{index}")));
            let item_index = positive_i64(obj.get(&format!("CreateItemIndex{index}")));
            let probability = positive_i64(obj.get(&format!("CreateItemProbability{index}")));
            if let (Some(item_type), Some(item_index), Some(probability)) =
                (item_type, item_index, probability)
            {
                chuchan.push((item_type as u32, item_index as u32, probability as u32));
            }
        }
        assert_eq!(chuchan, vec![(5, 89_346, 1_006_633), (5, 89_347, 41_943)]);
        // 权重和 = 2^20（普通 96% + 精制 4%）
        assert_eq!(chuchan.iter().map(|(_, _, p)| *p).sum::<u32>(), 1_048_576);
        assert_eq!(positive_i64(obj.get("RequireLevel")), Some(50));
        assert_eq!(non_negative_i64(obj.get("CostVigor")), Some(75));
    }

    #[test]
    fn parse_recipe_detail_n_dlc() {
        // 当前资料片配方：nDlc=9
        let text = include_str!("../tests/fixtures/detail_cooking_437.json");
        let value: Value = serde_json::from_str(text).expect("fixture 解析失败");
        let detail =
            parse_recipe_detail("cooking", 437, "云芝竹沥羹", 50, &value).expect("解析失败");
        assert_eq!(detail.n_dlc, Some(9));
        assert_eq!(detail.tab, "cooking");
        assert_eq!(detail.game_id, 437);
        assert_eq!(detail.name, "云芝竹沥羹");
        assert_eq!(detail.cailiao.len(), 5);
        assert_eq!(detail.chuchan.len(), 2);

        // 旧配方（饺子皮）：nDlc 缺省
        let text = include_str!("../tests/fixtures/detail_cooking_120.json");
        let value: Value = serde_json::from_str(text).expect("fixture 解析失败");
        let detail = parse_recipe_detail("cooking", 120, "饺子皮", 1, &value).expect("解析失败");
        assert_eq!(detail.n_dlc, None);
        assert_eq!(detail.name, "饺子皮");

        // 家具（梓匠）：nDlc 缺省但按规则全保留
        let text = include_str!("../tests/fixtures/detail_furniture_1.json");
        let value: Value = serde_json::from_str(text).expect("fixture 解析失败");
        let detail =
            parse_recipe_detail("furniture", 1, "柳叶水甘草", 9, &value).expect("解析失败");
        assert_eq!(detail.n_dlc, None);
        assert_eq!(detail.chuchan.len(), 1);
    }

    #[test]
    fn parse_item_and_missing_item() {
        let text = include_str!("../tests/fixtures/item_5_30853.json");
        let value: Value = serde_json::from_str(text).expect("fixture 解析失败");
        let obj = item_object(value).expect("物品应能解析");
        assert_eq!(str_field(&obj, "Name").as_deref(), Some("珠贝母"));
        assert_eq!(str_field(&obj, "TypeLabel").as_deref(), Some("材料"));
        assert_eq!(non_negative_i64(obj.get("Price")), Some(1000));

        // 不存在的物品：接口返回 []
        let missing: Value = serde_json::from_str("[]").expect("json");
        assert!(item_object(missing).is_none());
    }

    #[test]
    fn into_list_handles_wrapped() {
        let list = into_list(serde_json::json!({"code": 0, "data": [1, 2]}));
        assert_eq!(list.as_array().map(Vec::len), Some(2));
        let list = into_list(serde_json::json!([1]));
        assert_eq!(list.as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn tab_mapping() {
        assert_eq!(jiyi_of_tab("cooking"), Some(Jiyi::Pengren));
        assert_eq!(jiyi_of_tab("medicine"), Some(Jiyi::Zhiyao));
        assert_eq!(jiyi_of_tab("tailoring"), Some(Jiyi::Fengren));
        assert_eq!(jiyi_of_tab("founding"), Some(Jiyi::Zhuzhao));
        assert_eq!(jiyi_of_tab("furniture"), Some(Jiyi::Zijiang));
        assert_eq!(jiyi_of_tab("nope"), None);
        assert_eq!(item_id(5, 30_853), "5_30853");
    }
}
