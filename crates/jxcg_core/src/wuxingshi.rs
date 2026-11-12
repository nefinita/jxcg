//! 五行石面板合成（一键合成到 6 级）计算。
//!
//! 口径（PLAN.md §6.3 / 2026-11 面板数据，详见 data/wuxingshi.toml）：
//! - 按 `[[fangan]]` 取合成方案，从目标物品沿 `[[hecheng]]` 递归展开：
//!   主产物每次期望产出 = Σ(bp/10000 × shuliang)（主产物结果槽），
//!   `合成次数 = 需求 / 主产物期望`；失败（余量概率）通过放大次数体现，不额外损失材料；
//! - 方案内没有合成配方的物品视为采购原料，按 行情/手填（`jiage`）> 商店价 > 缺价 取价并聚合；
//! - 副产物 = 非主产物的结果槽：期望量 = 次数 × bp/10000 × shuliang，
//!   按行情/商店价折价，汇总为 `fuchanpin_shouru` 冲抵成本（缺价按 0 并进 `quejia`）；
//! - 合 6 意外出 8 级：目标为六级时，六级配方中 [`BAOJI_WUPIN_ID`]（八级）槽的 bp
//!   可由 `req.baoji_bp` 覆盖（None = 用数据值，0 = 忽略），按「额外产出」计入副产物收益；
//! - 期望量用 f64（合成次数/颗数可小数），**金额在结果字段上一次性 HALF_UP 到铜**；
//!   `zong_chengben = 材料 + 精力折金 + 加工费 − 副产物收入` 严格成立；
//! - `jingli = round(Σ 次数 × 每步精力)`；`danjia = round(zong_chengben / 目标件数)`；
//!   `baobenjia` 用 fee 模块由单位净成本反推（含 5% 手续费）；
//!   `lirun = 市场价 × 数量 − 手续费 − 净成本`（保管费按挂单规则默认 0）。
//!
//! 金额全程整数：f64 只承载「期望量」，最后一次性取整到铜；禁止用浮点表示已确定的金额。

use std::collections::{BTreeMap, BTreeSet};

use crate::{JxcgError, Result, fee, math, price};
use jxcg_api::{
    CailiaoMingxi, Catalog, Tong, WuxingshiBuzhou, WuxingshiFuchanpin, WuxingshiHecheng,
    WuxingshiJieguo, WuxingshiLianRequest, WuxingshiLianResult,
};

/// 六级五行石物品 id（默认目标）。
pub const LIUJI_WUPIN_ID: &str = "5_24428";
/// 八级五行石物品 id（合 6 意外爆 8 级的覆盖目标）。
pub const BAOJI_WUPIN_ID: &str = "5_24430";

/// 递归展开深度上限（防御脏数据成环；面板链实际 4 层）。
const MAX_DEPTH: usize = 16;

/// 单步合成公式的累计中间量（f64 期望量；金额最后统一取整）。
struct Fenxi<'a> {
    /// 主产物物品 id → 采用的单步配方（同物品多配方取 id 最小者）
    recipes: BTreeMap<&'a str, &'a WuxingshiHecheng>,
    /// 单步配方 id → 期望合成次数
    times: BTreeMap<&'a str, f64>,
    /// 采购原料 id → 期望数量
    leafs: BTreeMap<&'a str, f64>,
    /// 副产物 id → 期望数量
    byproducts: BTreeMap<&'a str, f64>,
    /// 精力合计（点，期望值）
    jingli: f64,
    /// 加工费合计（铜，期望值）
    jiagongfei: f64,
    /// 覆盖「合 6 出 8 级」概率（万分比）
    baoji_bp: Option<i64>,
}

impl<'a> Fenxi<'a> {
    fn new(recipes: BTreeMap<&'a str, &'a WuxingshiHecheng>, baoji_bp: Option<i64>) -> Self {
        Self {
            recipes,
            times: BTreeMap::new(),
            leafs: BTreeMap::new(),
            byproducts: BTreeMap::new(),
            jingli: 0.0,
            jiagongfei: 0.0,
            baoji_bp,
        }
    }

    /// 某物品需要 `qty` 件：有配方 → 递归展开；无配方 → 记为采购原料。
    fn add_need(
        &mut self,
        wupin_id: &'a str,
        qty: f64,
        depth: usize,
        stack: &mut Vec<&'a str>,
    ) -> Result<()> {
        if qty <= 0.0 {
            // 0 或负（脏数据）按 0 处理
            return Ok(());
        }
        if !qty.is_finite() {
            return Err(JxcgError::Data(format!(
                "五行石合成期望数量溢出：{wupin_id}"
            )));
        }
        match self.recipes.get(wupin_id).copied() {
            Some(recipe) => self.add_recipe(recipe, qty, false, depth, stack),
            None => {
                *self.leafs.entry(wupin_id).or_insert(0.0) += qty;
                Ok(())
            }
        }
    }

    /// 展开一个单步配方：`qty` 是需要的该配方主产物件数。
    fn add_recipe(
        &mut self,
        recipe: &'a WuxingshiHecheng,
        qty: f64,
        is_target: bool,
        depth: usize,
        stack: &mut Vec<&'a str>,
    ) -> Result<()> {
        let key = recipe.chanpin.as_str();
        if depth > MAX_DEPTH || stack.contains(&key) {
            return Err(JxcgError::Data(format!(
                "五行石合成配方成环或层级过深：{}",
                recipe.id
            )));
        }

        // 主产物每次期望产出 = Σ(bp/10000 × shuliang)
        let mut p_main: f64 = 0.0;
        for jieguo in &recipe.jieguo {
            if jieguo.wupin_id == recipe.chanpin && jieguo.bp > 0 {
                p_main += jieguo.bp as f64 / 10_000.0 * jieguo.shuliang as f64;
            }
        }
        if p_main <= 0.0 {
            return Err(JxcgError::Data(format!(
                "五行石合成配方「{}」缺少主产物的成功概率（bp > 0）",
                recipe.id
            )));
        }
        let times = qty / p_main;
        *self.times.entry(recipe.id.as_str()).or_insert(0.0) += times;
        self.jingli += times * recipe.jingli as f64;
        self.jiagongfei += times * recipe.jiagongfei as f64;

        stack.push(key);
        for item in &recipe.cailiao {
            self.add_need(
                item.wupin_id.as_str(),
                times * item.shuliang as f64,
                depth + 1,
                stack,
            )?;
        }
        for jieguo in &recipe.jieguo {
            if jieguo.wupin_id == recipe.chanpin {
                continue;
            }
            let bp = jieguo_bp(jieguo, is_target, self.baoji_bp);
            if bp <= 0 {
                continue;
            }
            let qiwang = times * bp as f64 / 10_000.0 * jieguo.shuliang as f64;
            *self
                .byproducts
                .entry(jieguo.wupin_id.as_str())
                .or_insert(0.0) += qiwang;
        }
        stack.pop();
        Ok(())
    }
}

/// 结果槽实际采用的概率（万分比）：目标为六级时可覆盖八级「意外爆出」槽。
fn jieguo_bp(jieguo: &WuxingshiJieguo, is_target: bool, baoji_bp: Option<i64>) -> i64 {
    let raw = if is_target && jieguo.wupin_id == BAOJI_WUPIN_ID {
        baoji_bp.unwrap_or(jieguo.bp)
    } else {
        jieguo.bp
    };
    raw.clamp(0, 10_000)
}

fn wupin_mingcheng(catalog: &Catalog, wupin_id: &str) -> String {
    catalog
        .wupin
        .iter()
        .find(|w| w.id == wupin_id)
        .map(|w| w.mingcheng.clone())
        .unwrap_or_else(|| wupin_id.to_string())
}

/// f64 期望金额 → 铜（HALF_UP，一次性取整）。
fn round_tong(value: f64, what: &str) -> Result<Tong> {
    if !value.is_finite() {
        return Err(JxcgError::Data(format!(
            "{what}出现非有限数值（数据或价格异常）"
        )));
    }
    let rounded = value.round();
    if rounded < i64::MIN as f64 || rounded >= i64::MAX as f64 {
        return Err(JxcgError::Data(format!("{what}超出金额范围")));
    }
    Ok(rounded as Tong)
}

/// f64 期望数量 → u32（HALF_UP，越界报中文错误）。
fn round_u32(value: f64, what: &str) -> Result<u32> {
    if !value.is_finite() {
        return Err(JxcgError::Data(format!("{what}出现非有限数值（数据异常）")));
    }
    let rounded = value.round();
    if rounded < 0.0 || rounded > u32::MAX as f64 {
        return Err(JxcgError::Data(format!("{what}超出范围")));
    }
    Ok(rounded as u32)
}

/// 一键合成计算：目标物品 × 件数 → 期望材料/精力/加工费/副产物/净成本/利润。
pub fn jisuan_lian(catalog: &Catalog, req: &WuxingshiLianRequest) -> Result<WuxingshiLianResult> {
    if req.shuliang == 0 {
        return Err(JxcgError::Data("目标件数必须大于 0".into()));
    }
    if req.jingli_danjia < 0 {
        return Err(JxcgError::Money("精力单价不能为负".into()));
    }
    fee::validate_shouxufei(&req.shouxufei)?;

    let fangan = catalog
        .wuxingshi_fangan
        .iter()
        .find(|f| f.id == req.fangan_id)
        .ok_or_else(|| JxcgError::Data(format!("五行石合成方案不存在：{}", req.fangan_id)))?;

    // 方案内单步配方按主产物索引；同主产物多配方取 id 最小者（保证确定性）
    let mut recipes: BTreeMap<&str, &WuxingshiHecheng> = BTreeMap::new();
    for hecheng in &catalog.wuxingshi_hecheng {
        if hecheng.fangan != req.fangan_id {
            continue;
        }
        match recipes.get(hecheng.chanpin.as_str()) {
            Some(old) if old.id.as_str() <= hecheng.id.as_str() => {}
            _ => {
                recipes.insert(hecheng.chanpin.as_str(), hecheng);
            }
        }
    }
    if recipes.is_empty() {
        return Err(JxcgError::Data(format!(
            "五行石合成方案「{}」没有单步配方",
            req.fangan_id
        )));
    }
    let target_recipe = recipes
        .get(req.mubiao_wupin_id.as_str())
        .copied()
        .ok_or_else(|| {
            JxcgError::Data(format!(
                "合成方案「{}」中没有目标物品「{}」的合成配方",
                req.fangan_id, req.mubiao_wupin_id
            ))
        })?;

    // 八级概率覆盖只对「目标 = 六级」生效（其余目标忽略 baoji_bp）
    let baoji_bp = if req.mubiao_wupin_id == LIUJI_WUPIN_ID {
        req.baoji_bp
    } else {
        None
    };

    let mut fenxi = Fenxi::new(recipes, baoji_bp);
    let mut stack: Vec<&str> = Vec::new();
    fenxi.add_recipe(target_recipe, req.shuliang as f64, true, 0, &mut stack)?;

    // ---- 采购原料（一级石等） ----
    let mut quejia: Vec<String> = Vec::new();
    // 该请求没有手填覆盖字段（五行石页面直接给价格表），统一按 商店价 > 行情 > 缺价 取价
    let empty_shoudong = BTreeSet::new();
    let mut cailiao: Vec<CailiaoMingxi> = Vec::with_capacity(fenxi.leafs.len());
    let mut cailiao_chengben_f: f64 = 0.0;
    for (wupin_id, qty) in &fenxi.leafs {
        let (danjia, laiyuan) =
            price::qu_danjia(catalog, &req.jiage, &empty_shoudong, wupin_id, &mut quejia)?;
        let xiaojia_f = *qty * danjia as f64;
        cailiao_chengben_f += xiaojia_f;
        cailiao.push(CailiaoMingxi {
            wupin_id: (*wupin_id).to_string(),
            mingcheng: wupin_mingcheng(catalog, wupin_id),
            shuliang: round_u32(*qty, "材料期望数量")?,
            danjia,
            xiaojia: round_tong(xiaojia_f, "材料小计")?,
            laiyuan: laiyuan.to_string(),
            zhongjian: false,
        });
    }
    let cailiao_chengben = round_tong(cailiao_chengben_f, "材料成本")?;

    // ---- 副产物（含合 6 出 8 级） ----
    let mut fuchanpin: Vec<WuxingshiFuchanpin> = Vec::with_capacity(fenxi.byproducts.len());
    let mut fuchanpin_shouru_f: f64 = 0.0;
    for (wupin_id, qiwang) in &fenxi.byproducts {
        let (danjia, _laiyuan) =
            price::qu_danjia(catalog, &req.jiage, &empty_shoudong, wupin_id, &mut quejia)?;
        let shouru_f = *qiwang * danjia as f64;
        fuchanpin_shouru_f += shouru_f;
        fuchanpin.push(WuxingshiFuchanpin {
            wupin_id: (*wupin_id).to_string(),
            mingcheng: wupin_mingcheng(catalog, wupin_id),
            qiwang: *qiwang,
            danjia,
            shouru: round_tong(shouru_f, "副产物收入")?,
        });
    }
    let fuchanpin_shouru = round_tong(fuchanpin_shouru_f, "副产物收入")?;

    // ---- 精力 / 加工费 / 净成本 ----
    let jingli = round_u32(fenxi.jingli, "精力合计")?;
    let jingli_chengben = round_tong(fenxi.jingli * req.jingli_danjia as f64, "精力折金")?;
    let jiagongfei = round_tong(fenxi.jiagongfei, "加工费")?;
    let zong_chengben = cailiao_chengben
        .checked_add(jingli_chengben)
        .and_then(|v| v.checked_add(jiagongfei))
        .and_then(|v| v.checked_sub(fuchanpin_shouru))
        .ok_or_else(|| JxcgError::Data("五行石净成本溢出".into()))?;

    // ---- 单位成本 / 保本价 / 市场利润 ----
    let danjia = math::to_tong(
        math::round_div_half_up_signed(zong_chengben as i128, req.shuliang as i128),
        "单位净成本",
    )?;
    let baobenjia = if danjia > 0 {
        fee::baobenjia(danjia, &req.shouxufei, 0)?
    } else {
        0
    };
    let (shichangjia, _) = price::qu_danjia(
        catalog,
        &req.jiage,
        &empty_shoudong,
        &req.mubiao_wupin_id,
        &mut quejia,
    )?;
    let shouru_maichu = shichangjia
        .checked_mul(req.shuliang as Tong)
        .ok_or_else(|| JxcgError::Data("五行石售卖收入溢出".into()))?;
    let feiyong = fee::jisuan_shouxufei(shouru_maichu, &req.shouxufei);
    let lirun = shouru_maichu
        .checked_sub(feiyong)
        .and_then(|v| v.checked_sub(zong_chengben))
        .ok_or_else(|| JxcgError::Data("五行石净利润溢出".into()))?;
    let lirunlv_bp = if zong_chengben > 0 {
        math::to_tong(
            math::round_div_half_up_signed(lirun as i128 * 10_000, zong_chengben as i128),
            "利润率",
        )?
    } else {
        0
    };

    // ---- 步骤明细（按 data 顺序，仅含实际用到的配方） ----
    let mut buzhou: Vec<WuxingshiBuzhou> = Vec::new();
    for hecheng in &catalog.wuxingshi_hecheng {
        if hecheng.fangan != req.fangan_id {
            continue;
        }
        let Some(times) = fenxi.times.get(hecheng.id.as_str()).copied() else {
            continue;
        };
        if times <= 0.0 {
            continue;
        }
        buzhou.push(WuxingshiBuzhou {
            hecheng_id: hecheng.id.clone(),
            chanpin: hecheng.chanpin.clone(),
            mingcheng: wupin_mingcheng(catalog, &hecheng.chanpin),
            cishu: times,
            jingli: round_u32(times * hecheng.jingli as f64, "步骤精力")?,
            jiagongfei: round_tong(times * hecheng.jiagongfei as f64, "步骤加工费")?,
        });
    }

    quejia.sort();
    quejia.dedup();

    Ok(WuxingshiLianResult {
        fangan_id: fangan.id.clone(),
        fangan_mingcheng: fangan.mingcheng.clone(),
        mubiao_wupin_id: req.mubiao_wupin_id.clone(),
        mubiao_mingcheng: wupin_mingcheng(catalog, &req.mubiao_wupin_id),
        shuliang: req.shuliang,
        cailiao,
        cailiao_chengben,
        jingli,
        jingli_chengben,
        jiagongfei,
        fuchanpin,
        fuchanpin_shouru,
        zong_chengben,
        danjia,
        baobenjia,
        shichangjia,
        lirun,
        lirunlv_bp,
        buzhou,
        quejia,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jxcg_api::{Banben, Cailiao, Pinlei, Quzheng, Shouxufei, Wupin, WuxingshiFangan};

    fn wupin(id: &str, mingcheng: &str) -> Wupin {
        Wupin {
            id: id.into(),
            mingcheng: mingcheng.into(),
            pinlei: Pinlei::Wuxingshi,
            dui: None,
            shangdianjia: None,
            laiyuan: None,
        }
    }

    fn hecheng(
        id: &str,
        chanpin: &str,
        jingli: u32,
        jiagongfei: i64,
        cailiao: Vec<(&str, u32)>,
        jieguo: Vec<(&str, i64, u32)>,
    ) -> WuxingshiHecheng {
        WuxingshiHecheng {
            id: id.into(),
            fangan: "mianban".into(),
            chanpin: chanpin.into(),
            jingli,
            jiagongfei,
            cailiao: cailiao
                .into_iter()
                .map(|(wupin_id, shuliang)| Cailiao {
                    wupin_id: wupin_id.into(),
                    shuliang,
                })
                .collect(),
            jieguo: jieguo
                .into_iter()
                .map(|(wupin_id, bp, shuliang)| WuxingshiJieguo {
                    wupin_id: wupin_id.into(),
                    bp,
                    shuliang,
                })
                .collect(),
        }
    }

    /// 与 data/wuxingshi.toml 一致的 2026-11 面板配方（金标准数据）。
    fn panel_catalog() -> Catalog {
        Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("5_24423", "五行石（一级）"),
                wupin("5_24424", "五行石（二级）"),
                wupin("5_24425", "五行石（三级）"),
                wupin("5_24426", "五行石（四级）"),
                wupin("5_24427", "五行石（五级）"),
                wupin("5_24428", "五行石（六级）"),
                wupin("5_24430", "五行石（八级）"),
            ],
            peifang: vec![],
            wuxingshi: vec![],
            wuxingshi_fangan: vec![WuxingshiFangan {
                id: "mianban".into(),
                mingcheng: "面板配方（游戏内一键合成）".into(),
                note: "测试".into(),
            }],
            wuxingshi_hecheng: vec![
                // 三级：12 一级 + 10 精力 + 0.36 金；92.76% 三级 / 7.24% 二级
                hecheng(
                    "mianban_3",
                    "5_24425",
                    10,
                    3_600,
                    vec![("5_24423", 12)],
                    vec![("5_24425", 9_276, 1), ("5_24424", 724, 1)],
                ),
                // 四级：3 三级 + 7 一级 + 10 精力 + 1.4421 金；99.11% 四级 / 0.72% 五级
                hecheng(
                    "mianban_4",
                    "5_24426",
                    10,
                    14_421,
                    vec![("5_24425", 3), ("5_24423", 7)],
                    vec![("5_24426", 9_911, 1), ("5_24427", 72, 1)],
                ),
                // 五级：3 四级 + 2 三级 + 10 精力 + 5.3801 金；99.15% 五级 / 0.85% 六级
                hecheng(
                    "mianban_5",
                    "5_24427",
                    10,
                    53_801,
                    vec![("5_24426", 3), ("5_24425", 2)],
                    vec![("5_24427", 9_915, 1), ("5_24428", 85, 1)],
                ),
                // 六级：3 五级 + 2 四级 + 10 精力 + 19.9066 金；99.55% 六级 / 0.45% 失败
                hecheng(
                    "mianban_6",
                    "5_24428",
                    10,
                    199_066,
                    vec![("5_24427", 3), ("5_24426", 2)],
                    vec![("5_24428", 9_955, 1), ("5_24430", 0, 1)],
                ),
            ],
            fuwuqi: vec![],
            zhiji: vec![],
        }
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
            mubiao_wupin_id: LIUJI_WUPIN_ID.into(),
            shuliang: 1,
            jingli_danjia: 0,
            shouxufei: cfg(),
            jiage: BTreeMap::new(),
            baoji_bp: None,
        }
    }

    #[test]
    fn golden_quantities_per_six_level() {
        // 面板金标准：每颗六级 ≈ 592.95 一级石 / 581.35 精力 / 679,703.94 铜加工费。
        // 一级石按 1 金/颗计价，用材料成本反推精确数量。
        let cat = panel_catalog();
        let mut req = request();
        req.jiage.insert("5_24423".to_string(), 10_000);
        let r = jisuan_lian(&cat, &req).unwrap();

        assert_eq!(r.fangan_id, "mianban");
        assert_eq!(r.fangan_mingcheng, "面板配方（游戏内一键合成）");
        assert_eq!(r.mubiao_wupin_id, "5_24428");
        assert_eq!(r.mubiao_mingcheng, "五行石（六级）");
        assert_eq!(r.shuliang, 1);

        // 采购原料只有一级石；展示数量 HALF_UP 到 593
        assert_eq!(r.cailiao.len(), 1);
        assert_eq!(r.cailiao[0].wupin_id, "5_24423");
        assert_eq!(r.cailiao[0].shuliang, 593);
        assert_eq!(r.cailiao[0].danjia, 10_000);
        assert_eq!(r.cailiao[0].laiyuan, "hangqing");
        assert!(!r.cailiao[0].zhongjian);
        // 成本 5,929,528 铜 → 期望数量 592.9528 颗
        assert_eq!(r.cailiao_chengben, 5_929_528);
        let qty = r.cailiao_chengben as f64 / 10_000.0;
        assert!((qty - 592.952_815).abs() < 1e-4, "一级石期望 {qty}");
        // 精力 581.346 → 581；加工费 679,703.94 → 679,704
        assert_eq!(r.jingli, 581);
        assert_eq!(r.jiagongfei, 679_704);
        assert_eq!(r.jingli_chengben, 0);

        // 副产物期望：二级 3.103322 / 五级 0.0808355 / 六级 0.0258349；八级默认 0 不出现
        let by: BTreeMap<&str, f64> = r
            .fuchanpin
            .iter()
            .map(|f| (f.wupin_id.as_str(), f.qiwang))
            .collect();
        assert!((by["5_24424"] - 3.103_322_016).abs() < 1e-6);
        assert!((by["5_24427"] - 0.080_835_480).abs() < 1e-9);
        assert!((by["5_24428"] - 0.025_834_865).abs() < 1e-9);
        assert!(!by.contains_key("5_24430"));

        // 四个步骤按 data 顺序；次数/精力/加工费为各步骤期望合计
        assert_eq!(r.buzhou.len(), 4);
        let steps: BTreeMap<&str, &WuxingshiBuzhou> = r
            .buzhou
            .iter()
            .map(|b| (b.hecheng_id.as_str(), b))
            .collect();
        assert_eq!(r.buzhou[0].hecheng_id, "mianban_3");
        assert_eq!(r.buzhou[3].hecheng_id, "mianban_6");
        assert!((steps["mianban_3"].cishu - 42.863_563_756).abs() < 1e-6);
        assert!((steps["mianban_4"].cishu - 11.227_149_987).abs() < 1e-6);
        assert!((steps["mianban_5"].cishu - 3.039_395_890).abs() < 1e-6);
        assert!((steps["mianban_6"].cishu - 1.004_520_342).abs() < 1e-6);
        assert_eq!(steps["mianban_3"].jingli, 429);
        assert_eq!(steps["mianban_3"].jiagongfei, 154_309);
        assert_eq!(steps["mianban_6"].jingli, 10);
        assert_eq!(steps["mianban_6"].jiagongfei, 199_966);

        // 总成本严格对齐（价格全缺 → 副产物收入 0；目标缺价）
        assert_eq!(
            r.zong_chengben,
            r.cailiao_chengben + r.jingli_chengben + r.jiagongfei - r.fuchanpin_shouru
        );
        assert_eq!(r.zong_chengben, 5_929_528 + 679_704);
        assert_eq!(r.danjia, r.zong_chengben);
        assert_eq!(r.shichangjia, 0);
        assert_eq!(r.lirun, -r.zong_chengben);
        // 缺价：三个副产物 + 目标（一级石已补价）
        assert_eq!(
            r.quejia,
            vec![
                "5_24424".to_string(),
                "5_24427".to_string(),
                "5_24428".to_string()
            ]
        );
    }

    #[test]
    fn golden_cost_and_profit() {
        let cat = panel_catalog();
        let mut req = request();
        req.jingli_danjia = 100; // 1 点精力 = 1 银
        req.jiage = BTreeMap::from([
            ("5_24423".to_string(), 10_000),      // 一级 1 金
            ("5_24424".to_string(), 20_000),      // 二级 2 金
            ("5_24427".to_string(), 1_000_000),   // 五级 100 金
            ("5_24428".to_string(), 2_000_000),   // 六级 200 金（目标市场价）
            ("5_24430".to_string(), 255_000_000), // 八级 25500 金
        ]);
        let r = jisuan_lian(&cat, &req).unwrap();
        // 材料 592.9528×10000 = 5,929,528
        assert_eq!(r.cailiao_chengben, 5_929_528);
        // 精力 581.3463×100 = 58,134.63 → 58,135
        assert_eq!(r.jingli_chengben, 58_135);
        assert_eq!(r.jiagongfei, 679_704);
        // 副产物收入 = round(3.103322×20000 + 0.0808355×1,000,000 + 0.0258349×2,000,000)
        assert_eq!(r.fuchanpin_shouru, 194_572);
        assert_eq!(
            r.fuchanpin.iter().map(|f| f.shouru).sum::<i64>(),
            62_066 + 80_835 + 51_670
        );
        let zong = 5_929_528 + 58_135 + 679_704 - 194_572;
        assert_eq!(r.zong_chengben, zong);
        assert_eq!(r.danjia, zong);
        // 保本价：fee 模块由单位净成本反推，满足售价 − 5% 手续费 >= 单位成本且逐铜最小
        let p = r.baobenjia;
        let net = |price: i64| price - fee::jisuan_shouxufei(price, &cfg());
        assert!(net(p) >= r.danjia);
        assert!(net(p - 1) < r.danjia);
        assert_eq!(p, 6_813_468);
        // 市场价 200 金：收入 2,000,000 − 手续费 100,000 − 净成本
        assert_eq!(r.shichangjia, 2_000_000);
        assert_eq!(r.lirun, 2_000_000 - 100_000 - zong);
        assert_eq!(r.lirunlv_bp, -7_065); // −7064.64 万分比，负数 HALF_UP 远离 0
        assert!(r.quejia.is_empty());
    }

    #[test]
    fn shuliang_scaling() {
        let cat = panel_catalog();
        let mut req = request();
        req.shuliang = 3;
        req.jiage = BTreeMap::from([
            ("5_24423".to_string(), 10_000),
            ("5_24424".to_string(), 20_000),
            ("5_24427".to_string(), 1_000_000),
            ("5_24428".to_string(), 2_000_000),
        ]);
        let r3 = jisuan_lian(&cat, &req).unwrap();
        assert_eq!(r3.cailiao[0].shuliang, 1_779); // 1778.86 → 1779
        assert_eq!(r3.jingli, 1_744); // 1744.04 → 1744
        // 单件结果 ×3 与批量结果在取整误差内一致
        let mut one = req.clone();
        one.shuliang = 1;
        let r1 = jisuan_lian(&cat, &one).unwrap();
        assert!((r3.cailiao_chengben - r1.cailiao_chengben * 3).abs() <= 2);
        assert!((r3.zong_chengben - r1.zong_chengben * 3).abs() <= 8);
        assert!((r3.danjia - r1.danjia).abs() <= 2);
        assert_eq!(r3.shuliang, 3);
        assert_eq!(r3.cailiao[0].shuliang, 1_779);
    }

    #[test]
    fn success_rate_amplifies() {
        // 把六级主产物概率从 99.55% 改成 50%：期望次数正好翻倍（1/0.5 = 2）
        let mut cat = panel_catalog();
        if let Some(recipe) = cat
            .wuxingshi_hecheng
            .iter_mut()
            .find(|h| h.id == "mianban_6")
            && let Some(main) = recipe.jieguo.first_mut()
        {
            main.bp = 5_000;
        }
        let mut req = request();
        req.jiage.insert("5_24423".to_string(), 10_000);
        let r = jisuan_lian(&cat, &req).unwrap();
        let step6 = r
            .buzhou
            .iter()
            .find(|b| b.hecheng_id == "mianban_6")
            .unwrap();
        assert!((step6.cishu - 2.0).abs() < 1e-9);
        // 六级要 2 次 → 五级/四级需求放大，一级石必然多于 99.55% 情形
        assert!(r.cailiao[0].shuliang > 593);
        assert!(r.jingli > 581);
        // 副产物也按放大后的次数增加（六级副产物来自 mianban_5 的 0.85%）
        let by6 = r
            .fuchanpin
            .iter()
            .find(|f| f.wupin_id == "5_24428")
            .map(|f| f.qiwang)
            .unwrap_or(0.0);
        assert!(by6 > 0.05);
    }

    #[test]
    fn baoji_bp_override() {
        // 合 6 出 8 级：0.05% → 期望 +12.8 金/颗；0.1% → +25.6 金/颗（八级 25500 金）
        let cat = panel_catalog();
        let base_price = BTreeMap::from([
            ("5_24423".to_string(), 10_000),
            ("5_24428".to_string(), 2_000_000),
            ("5_24430".to_string(), 255_000_000),
        ]);
        let run = |baoji_bp: Option<i64>| {
            let mut req = request();
            req.jiage = base_price.clone();
            req.baoji_bp = baoji_bp;
            jisuan_lian(&cat, &req).unwrap()
        };
        let none = run(None);
        // 数据 bp=0 + 无覆盖 → 没有八级收益（六级副产物仍按行情折价，故基线收入非 0）
        assert!(none.fuchanpin.iter().all(|f| f.wupin_id != "5_24430"));

        let half = run(Some(5));
        let delta_half = half.fuchanpin_shouru - none.fuchanpin_shouru;
        assert!(
            (delta_half - 128_076).abs() <= 1,
            "0.05% 期望收益增量 {delta_half} 应约 128,076 铜（12.8 金）"
        );
        let eight = half
            .fuchanpin
            .iter()
            .find(|f| f.wupin_id == "5_24430")
            .expect("应出现八级副产物");
        assert!((eight.qiwang - 0.000_502_260).abs() < 1e-9);
        assert_eq!(eight.shouru, 128_076);

        let one_percent = run(Some(10));
        let delta_one = one_percent.fuchanpin_shouru - none.fuchanpin_shouru;
        assert!(
            (delta_one - 256_153).abs() <= 1,
            "0.1% 期望收益增量 {delta_one} 应约 256,153 铜（25.6 金）"
        );
        // 目标不是六级时忽略覆盖（五级配方没有八级槽）
        let mut req5 = request();
        req5.mubiao_wupin_id = "5_24427".into();
        req5.jiage = base_price.clone();
        req5.baoji_bp = Some(10);
        let r5 = jisuan_lian(&cat, &req5).unwrap();
        assert!(r5.fuchanpin.iter().all(|f| f.wupin_id != "5_24430"));
    }

    #[test]
    fn npc_shop_price_priority() {
        // 一级石/副产物设为 NPC 有售：行情价不参与，全部按商店价
        let mut cat = panel_catalog();
        for (id, shop) in [
            ("5_24423", 5_000_i64),
            ("5_24424", 20_000),
            ("5_24427", 1_000_000),
            ("5_24428", 2_000_000),
        ] {
            if let Some(wupin) = cat.wupin.iter_mut().find(|w| w.id == id) {
                wupin.shangdianjia = Some(shop);
            }
        }
        let mut req = request();
        req.jiage = BTreeMap::from([
            ("5_24423".to_string(), 10_000),
            ("5_24424".to_string(), 1),
            ("5_24427".to_string(), 1),
            ("5_24428".to_string(), 1),
        ]);
        let r = jisuan_lian(&cat, &req).unwrap();
        // 一级：592.9528 × 5000 = 2,964,764 铜
        assert_eq!(r.cailiao[0].danjia, 5_000);
        assert_eq!(r.cailiao[0].laiyuan, "shangdian");
        assert_eq!(r.cailiao_chengben, 2_964_764);
        // 副产物按商店价折价：总额一次性取整 = 194,572；行内小计各自取整，合计 194,571
        assert_eq!(r.fuchanpin_shouru, 194_572);
        assert_eq!(
            r.fuchanpin.iter().map(|f| f.shouru).sum::<i64>(),
            62_066 + 80_835 + 51_670
        );
        let by2 = r
            .fuchanpin
            .iter()
            .find(|f| f.wupin_id == "5_24424")
            .expect("二级副产物");
        assert_eq!(by2.danjia, 20_000);
        assert_eq!(
            r.zong_chengben,
            r.cailiao_chengben + r.jingli_chengben + r.jiagongfei - r.fuchanpin_shouru
        );
    }

    #[test]
    fn quejia_propagation() {
        // 空价格表：一级石/副产物/目标全部缺价 → 按 0 计，quejia 完整传播且不报错
        let cat = panel_catalog();
        let req = request();
        let r = jisuan_lian(&cat, &req).unwrap();
        assert_eq!(r.cailiao_chengben, 0);
        assert_eq!(r.fuchanpin_shouru, 0);
        assert_eq!(r.shichangjia, 0);
        assert_eq!(r.zong_chengben, r.jiagongfei);
        assert_eq!(
            r.quejia,
            vec![
                "5_24423".to_string(),
                "5_24424".to_string(),
                "5_24427".to_string(),
                "5_24428".to_string()
            ]
        );
        assert!(r.cailiao[0].laiyuan == "quejia");
    }

    #[test]
    fn errors() {
        let cat = panel_catalog();
        let mut req = request();
        req.fangan_id = "nope".into();
        assert!(jisuan_lian(&cat, &req).is_err());

        let mut req = request();
        req.mubiao_wupin_id = "5_24424".into(); // 二级没有合成配方
        assert!(jisuan_lian(&cat, &req).is_err());

        let mut req = request();
        req.shuliang = 0;
        assert!(jisuan_lian(&cat, &req).is_err());

        let mut req = request();
        req.jingli_danjia = -1;
        assert!(jisuan_lian(&cat, &req).is_err());

        let mut req = request();
        req.shouxufei.shuilv_bp = 10_000;
        assert!(jisuan_lian(&cat, &req).is_err());

        let mut req = request();
        req.jiage.insert("5_24423".to_string(), -1);
        assert!(jisuan_lian(&cat, &req).is_err());

        // 缺少主产物概率
        let mut cat_bad = panel_catalog();
        if let Some(recipe) = cat_bad
            .wuxingshi_hecheng
            .iter_mut()
            .find(|h| h.id == "mianban_6")
        {
            recipe.jieguo = vec![WuxingshiJieguo {
                wupin_id: "5_24430".into(),
                bp: 5,
                shuliang: 1,
            }];
        }
        let mut req = request();
        req.jiage = BTreeMap::from([("5_24423".to_string(), 1)]);
        assert!(jisuan_lian(&cat_bad, &req).is_err());

        // 成环：三级配方改为吃五级，五级吃三级
        let mut cat_cycle = panel_catalog();
        for recipe in &mut cat_cycle.wuxingshi_hecheng {
            match recipe.id.as_str() {
                "mianban_3" => {
                    recipe.cailiao = vec![Cailiao {
                        wupin_id: "5_24427".into(),
                        shuliang: 1,
                    }];
                }
                "mianban_5" => {
                    recipe.cailiao = vec![Cailiao {
                        wupin_id: "5_24425".into(),
                        shuliang: 1,
                    }];
                }
                _ => {}
            }
        }
        assert!(jisuan_lian(&cat_cycle, &request()).is_err());
    }

    #[test]
    fn f64_rounding_helpers() {
        assert_eq!(round_tong(592.5, "x").unwrap(), 593);
        assert_eq!(round_tong(592.4999, "x").unwrap(), 592);
        assert_eq!(round_tong(10.5, "x").unwrap(), 11);
        assert_eq!(round_tong(-1.5, "x").unwrap(), -2);
        assert_eq!(round_tong(0.0, "x").unwrap(), 0);
        assert!(round_tong(f64::INFINITY, "x").is_err());
        assert!(round_tong(f64::NAN, "x").is_err());
        assert!(round_tong(1e30, "x").is_err());
        assert_eq!(round_u32(592.5, "x").unwrap(), 593);
        assert_eq!(round_u32(0.4, "x").unwrap(), 0);
        assert!(round_u32(-0.5, "x").is_err());
        assert!(round_u32(f64::NAN, "x").is_err());
    }

    #[test]
    fn formula_is_finite_with_extreme_prices() {
        // 极端价格/精力单价不产生 NaN/Inf，结果仍可序列化
        let cat = panel_catalog();
        let mut req = request();
        req.jingli_danjia = 9_000_000_000;
        req.jiage = BTreeMap::from([
            ("5_24423".to_string(), 1_000_000_000_000),
            ("5_24428".to_string(), 1_000_000_000_000),
        ]);
        let r = jisuan_lian(&cat, &req).unwrap();
        assert!(r.cailiao_chengben > 0);
        assert!(r.zong_chengben > 0);
        assert_eq!(
            r.zong_chengben,
            r.cailiao_chengben + r.jingli_chengben + r.jiagongfei - r.fuchanpin_shouru
        );
    }
}
