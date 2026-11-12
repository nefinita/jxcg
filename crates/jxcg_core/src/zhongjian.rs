//! 中间产物索引与自制成本展开（PLAN.md §6.3.1）。
//!
//! 定义：某物品同时是「某配方的产物」与「某配方的材料」→ **中间产物**；
//! 只产这类物品的配方称为「中间产物配方」。
//!
//! 展开口径：
//! - 中间产物不按行情价计入最终配方成本，而是按自制成本展开：
//!   自身材料（递归，采购原料仍按 行情/手填 > 商店价 > 缺价）+ 精力 × 精力单价 + 制作费，
//!   除以「每批产出的期望件数」得到**单位成本（HALF_UP 到铜）**，上层按「单位成本 × 数量」摊入；
//! - 中间产物自制不挂单，**不计保管费**；最终配方的保管费照旧；
//! - 生产者选择：同一物品存在多个产出配方时，取 `(dengji, id)` 最小者
//!   （等级低者优先，同等级按 id 升序，保证结果确定）；
//! - 手填覆盖优先：物品 id 在 `shoudong` 集合中 → 按 `jiage` 手填价采购，不展开；
//! - 兜底：无产出配方 / 期望产出为 0 / 成环 / 递归深度超限（> [`MAX_DEPTH`]）/ 数值溢出
//!   → 该行回退采购价（`jiage` → 商店价 → 缺价）；
//! - 缺价：递归里任何采购原料缺价 → 按 0 计入并汇总到 `quejia`（去重由调用方统一做）。
//!
//! 金额全程整数：成本用 i128 分子/分母精确计算，最后 HALF_UP 到铜；禁止 f64 参与金额。

use std::collections::{BTreeMap, BTreeSet};

use crate::math;
use jxcg_api::{Catalog, Peifang, QUANZHONG_FENMU, Tong};

/// 期望产出分母：每槽贡献 `quanzhong × (min+max) / (2 × 2^20)` 件。
pub(crate) const QIWANG_DEN: i128 = 2 * QUANZHONG_FENMU as i128;

/// 递归展开深度上限（数据实测最大约 3 层，留足冗余；超限回退采购价）。
pub const MAX_DEPTH: usize = 8;

/// 中间产物索引（由 Catalog 派生一次，计算/排行请求内复用）。
pub struct ZhongjianIndex<'a> {
    catalog: &'a Catalog,
    /// 中间产物物品 id 集合（产物 ∩ 材料）
    ids: BTreeSet<&'a str>,
    /// 物品 id → 采用的产出配方
    producers: BTreeMap<&'a str, &'a Peifang>,
}

impl<'a> ZhongjianIndex<'a> {
    /// 从目录派生索引（O(配方数 × 槽数 + log n)）。
    pub fn new(catalog: &'a Catalog) -> Self {
        let mut cailiao: BTreeSet<&str> = BTreeSet::new();
        for peifang in &catalog.peifang {
            for item in &peifang.cailiao {
                cailiao.insert(item.wupin_id.as_str());
            }
        }
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        let mut producers: BTreeMap<&str, &'a Peifang> = BTreeMap::new();
        for peifang in &catalog.peifang {
            for slot in &peifang.chuchan {
                let id = slot.wupin_id.as_str();
                if !cailiao.contains(id) {
                    continue;
                }
                ids.insert(id);
                match producers.get(id) {
                    Some(old) => {
                        // 同物品多配方：取 (dengji, id) 最小者
                        if (peifang.dengji, peifang.id.as_str()) < (old.dengji, old.id.as_str()) {
                            producers.insert(id, peifang);
                        }
                    }
                    None => {
                        producers.insert(id, peifang);
                    }
                }
            }
        }
        Self {
            catalog,
            ids,
            producers,
        }
    }

    pub fn catalog(&self) -> &'a Catalog {
        self.catalog
    }

    /// 某物品是否为中间产物。
    pub fn is_zhongjian(&self, wupin_id: &str) -> bool {
        self.ids.contains(wupin_id)
    }

    /// 中间产物采用的产出配方。
    pub fn producer(&self, wupin_id: &str) -> Option<&'a Peifang> {
        self.producers.get(wupin_id).copied()
    }

    /// 该配方是否「只产中间产物」（产物非空且每个产物槽都是中间产物）。
    pub fn is_zhongjian_peifang(&self, peifang: &Peifang) -> bool {
        !peifang.chuchan.is_empty()
            && peifang
                .chuchan
                .iter()
                .all(|slot| self.ids.contains(slot.wupin_id.as_str()))
    }

    /// 中间产物数量（测试/展示用）。
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// 中间产物配方「一批产出」需要的一层材料（金额已确定到铜）。
#[derive(Debug, Clone)]
pub(crate) struct BatchCailiao {
    pub wupin_id: String,
    /// 每批产出的需求数量
    pub shuliang: u32,
    /// 采购/更深自制单价（铜）
    pub danjia: Tong,
    /// 是否更深一层自制中间产物
    pub zhongjian: bool,
    /// hangqing / shoudong / shangdian / quejia / zizhi
    pub laiyuan: String,
}

/// 中间产物单位自制成本（HALF_UP 到铜）+ 计算明细。
#[derive(Debug, Clone)]
pub(crate) struct ZizhiUnit {
    /// 采用的产出配方 id
    pub peifang_id: String,
    /// 单位自制成本（铜）
    pub danjia: Tong,
    /// 每单位该中间产物的链路精力（HALF_UP 到整数）
    pub unit_jingli: u32,
    /// 每批期望产出 = `q_num / QIWANG_DEN`
    pub q_num: i128,
    /// 每批产出的一层材料
    pub cailiao: Vec<BatchCailiao>,
    /// 该单位成本链路上缺价的采购原料 id（memo 命中时也要能冒泡给调用方）
    pub quejia: Vec<String>,
}

/// 请求级价格上下文（`jiage` 为全量行情+手填价；`shoudong` 为手填覆盖 id）。
pub(crate) struct PriceCtx<'a> {
    pub jiage: &'a BTreeMap<String, Tong>,
    pub shoudong: &'a BTreeSet<String>,
    pub jingli_danjia: Tong,
}

/// 展开某中间产物的单位自制成本。
///
/// - `memo`：请求内单位成本缓存（同一 `PriceCtx` 下有效；排行等批量入口复用）；
/// - `quejia`：递归中缺价物品汇总（调用方负责排序去重）；
/// - 返回 `None` 表示不可展开（调用方回退采购价）。
pub(crate) fn expand_unit(
    index: &ZhongjianIndex<'_>,
    price: &PriceCtx<'_>,
    memo: &mut BTreeMap<String, ZizhiUnit>,
    wupin_id: &str,
    quejia: &mut Vec<String>,
) -> Option<ZizhiUnit> {
    let mut stack: Vec<String> = Vec::new();
    let unit = expand_inner(index, price, memo, wupin_id, &mut stack, 0)?;
    // 缺价随「单位成本」一起缓存：memo 命中时也要重复冒泡，避免排行等批量场景漏报
    quejia.extend(unit.quejia.iter().cloned());
    Some(unit)
}

#[allow(clippy::too_many_arguments)]
fn expand_inner(
    index: &ZhongjianIndex<'_>,
    price: &PriceCtx<'_>,
    memo: &mut BTreeMap<String, ZizhiUnit>,
    wupin_id: &str,
    stack: &mut Vec<String>,
    depth: usize,
) -> Option<ZizhiUnit> {
    if let Some(unit) = memo.get(wupin_id) {
        return Some(unit.clone());
    }
    if depth > MAX_DEPTH {
        return None;
    }
    // 成环：当前递归栈里已出现该物品 → 不可展开
    if stack.iter().any(|id| id == wupin_id) {
        return None;
    }
    let producer = index.producer(wupin_id)?;
    if producer.zhizuofei < 0 {
        return None;
    }

    // 每批期望产出 = Σ quanzhong × (min+max) / (2 × 2^20)
    let mut q_num: i128 = 0;
    for slot in &producer.chuchan {
        let span = slot.min as i128 + slot.max as i128;
        q_num = q_num.checked_add((slot.quanzhong as i128).checked_mul(span)?)?;
    }
    if q_num <= 0 {
        return None;
    }

    stack.push(wupin_id.to_string());
    let computed = accumulate_batch(index, price, memo, producer, q_num, stack, depth);
    // 无论成功失败都出栈，避免后续兄弟节点被误判成环
    stack.pop();

    if let Some(unit) = &computed {
        memo.insert(wupin_id.to_string(), unit.clone());
    }
    computed
}

/// 汇总一批产出的成本与精力（已入栈后调用；失败返回 None 由上层回退）。
#[allow(clippy::too_many_arguments)]
fn accumulate_batch(
    index: &ZhongjianIndex<'_>,
    price: &PriceCtx<'_>,
    memo: &mut BTreeMap<String, ZizhiUnit>,
    producer: &Peifang,
    q_num: i128,
    stack: &mut Vec<String>,
    depth: usize,
) -> Option<ZizhiUnit> {
    let mut batch_cost: i128 = 0;
    let mut chain_jingli: i128 = 0;
    let mut unit_quejia: Vec<String> = Vec::new();
    let mut rows: Vec<BatchCailiao> = Vec::with_capacity(producer.cailiao.len());

    for item in &producer.cailiao {
        // 中间产物默认递归自制；手填覆盖（shoudong）时按手填价采购
        let (danjia, laiyuan, zhongjian, unit_jingli) =
            if index.is_zhongjian(&item.wupin_id) && !price.shoudong.contains(&item.wupin_id) {
                match expand_inner(index, price, memo, &item.wupin_id, stack, depth + 1) {
                    Some(inner) => {
                        // 更深链路的缺价同样冒泡到本层
                        unit_quejia.extend(inner.quejia.iter().cloned());
                        (inner.danjia, "zizhi".to_string(), true, inner.unit_jingli)
                    }
                    None => {
                        // 展开失败：回退行情价 → 商店价 → 缺价
                        let (price_tong, source) =
                            purchase_price(index, price, &item.wupin_id, &mut unit_quejia);
                        (price_tong, source.to_string(), false, 0)
                    }
                }
            } else {
                let (price_tong, source) =
                    purchase_price(index, price, &item.wupin_id, &mut unit_quejia);
                (price_tong, source.to_string(), false, 0)
            };

        batch_cost =
            batch_cost.checked_add((item.shuliang as i128).checked_mul(danjia as i128)?)?;
        chain_jingli =
            chain_jingli.checked_add((item.shuliang as i128).checked_mul(unit_jingli as i128)?)?;
        rows.push(BatchCailiao {
            wupin_id: item.wupin_id.clone(),
            shuliang: item.shuliang,
            danjia,
            zhongjian,
            laiyuan,
        });
    }

    // 自身精力 + 制作费（中间产物自制不挂单 → 不计保管费）
    batch_cost = batch_cost
        .checked_add((producer.jingli as i128).checked_mul(price.jingli_danjia as i128)?)?;
    batch_cost = batch_cost.checked_add(producer.zhizuofei as i128)?;
    // 链路精力 = 自身精力 + 更深中间产物链路精力（精力单价为 0 时也照记，供每精力利润）
    chain_jingli = chain_jingli.checked_add(producer.jingli as i128)?;

    // 单位成本 = 每批成本 / 期望产出，HALF_UP 到铜
    let danjia = i64::try_from(math::round_div_half_up(
        batch_cost.checked_mul(QIWANG_DEN)?,
        q_num,
    ))
    .ok()?;
    // 单位链路精力 = 每批链路精力 / 期望产出，HALF_UP 到整数
    let unit_jingli = u32::try_from(math::round_div_half_up(
        chain_jingli.checked_mul(QIWANG_DEN)?,
        q_num,
    ))
    .ok()?;

    Some(ZizhiUnit {
        peifang_id: producer.id.clone(),
        danjia,
        unit_jingli,
        q_num,
        cailiao: rows,
        quejia: unit_quejia,
    })
}

/// 采购价：行情/手填（`jiage`）> 商店价 > 缺价。
///
/// `laiyuan`：手填（id ∈ shoudong）= `shoudong`；行情 = `hangqing`；商店 = `shangdian`；缺价 = `quejia`。
fn purchase_price(
    index: &ZhongjianIndex<'_>,
    price: &PriceCtx<'_>,
    wupin_id: &str,
    quejia: &mut Vec<String>,
) -> (Tong, &'static str) {
    if let Some(value) = price.jiage.get(wupin_id)
        && *value >= 0
    {
        let laiyuan = if price.shoudong.contains(wupin_id) {
            "shoudong"
        } else {
            "hangqing"
        };
        return (*value, laiyuan);
    }
    if let Some(wupin) = index.catalog().wupin.iter().find(|w| w.id == wupin_id)
        && let Some(value) = wupin.shangdianjia
        && value >= 0
    {
        return (value, "shangdian");
    }
    quejia.push(wupin_id.to_string());
    (0, "quejia")
}
