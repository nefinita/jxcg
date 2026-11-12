//! 单配方成本/利润计算。
//!
//! 口径（PLAN.md §6.3 / §6.3.1）：
//! - 材料成本 = Σ(需求数量 × 单价)；单价优先级：**行情/手填（`jiage`）> 商店价 > 缺价**，
//!   缺价按 0 计入并汇总到 `quejia`（供排行过滤与界面提示）；
//! - 中间产物（产物 ∩ 材料）在 `zhongjian_zizhi=true`（默认）时按**自制成本展开**：
//!   自身材料递归 + 精力×精力单价 + 制作费，按期望产出折算单位成本，摊入最终产物的材料成本；
//!   手填覆盖（id ∈ `shoudong`）的中间产物按手填价采购、不展开；`zhongjian_zizhi=false` 为旧行为；
//! - 期望产出 = Σ(权重 / 2^20 × (min+max)/2)，批量 `cishu` 次按期望线性累计；
//!   权重分母固定 [`QUANZHONG_FENMU`]（2^20），不重新归一化；
//! - 期望收入 = Σ(权重/2^20 × (min+max)/2 × cishu × 单价)：用 i128 精确累加，
//!   **最后一次性 HALF_UP 到铜**（逐槽取整会累计误差）；
//! - 手续费按期望收入 5% HALF_UP；保管费 = 单次保管费 × 挂单次数 × 批量（中间产物不挂单、不计保管费）；
//! - 净利润 = 税后收入 − 材料 − 精力 − 制作费 − 保管费；
//!   利润率万分比用整数 HALF_UP；每精力利润按（最终精力 + 中间产物精力）计算、整数截断。

use std::collections::{BTreeMap, BTreeSet};

use crate::zhongjian::{self, PriceCtx, QIWANG_DEN, ZhongjianIndex, ZizhiUnit};
use crate::{JxcgError, Result, fee, math};
use jxcg_api::{
    CailiaoMingxi, Catalog, Chuchan, ChuchanMingxi, JisuanRequest, JisuanResult, QUANZHONG_FENMU,
    Tong, Wupin, ZhongjianMingxi,
};

/// 单次制作的期望产出件数（按权重分母 2^20 计算，未做取整）。
pub fn chuchan_qiwang(chuchan: &[Chuchan]) -> f64 {
    chuchan
        .iter()
        .map(|slot| {
            let p = slot.quanzhong as f64 / QUANZHONG_FENMU as f64;
            p * (slot.min as f64 + slot.max as f64) / 2.0
        })
        .sum()
}

fn find_wupin<'a>(catalog: &'a Catalog, id: &str) -> Option<&'a Wupin> {
    catalog.wupin.iter().find(|w| w.id == id)
}

fn wupin_mingcheng(catalog: &Catalog, id: &str) -> String {
    find_wupin(catalog, id)
        .map(|w| w.mingcheng.clone())
        .unwrap_or_else(|| id.to_string())
}

/// 取采购单价：行情/手填 > 商店价 > 缺价。
///
/// 返回 `(单价, 来源标记)`；缺价返回 `(0, "quejia")` 并写入 `quejia`。
/// 来源标记：手填（id ∈ `shoudong`）= `shoudong`；行情 = `hangqing`；商店 = `shangdian`。
fn qu_danjia(
    catalog: &Catalog,
    jiage: &BTreeMap<String, Tong>,
    shoudong: &BTreeSet<String>,
    wupin_id: &str,
    quejia: &mut Vec<String>,
) -> Result<(Tong, &'static str)> {
    if let Some(price) = jiage.get(wupin_id) {
        if *price < 0 {
            return Err(JxcgError::Money(format!(
                "行情/手填单价不能为负：{wupin_id} = {price}"
            )));
        }
        let laiyuan = if shoudong.contains(wupin_id) {
            "shoudong"
        } else {
            "hangqing"
        };
        return Ok((*price, laiyuan));
    }
    if let Some(wupin) = find_wupin(catalog, wupin_id)
        && let Some(price) = wupin.shangdianjia
    {
        if price < 0 {
            return Err(JxcgError::Data(format!(
                "商店价不能为负：{wupin_id} = {price}"
            )));
        }
        return Ok((price, "shangdian"));
    }
    quejia.push(wupin_id.to_string());
    Ok((0, "quejia"))
}

/// 请求级批量计算器：复用中间产物索引与单位自制成本 memo。
///
/// 构造参数 `jiage` / `shoudong` / `jingli_danjia` 是**请求级价格上下文**；
/// 同一个计算器上的多次 [`Calculator::jisuan`] 必须使用同一价格上下文
/// （排行等批量入口满足该条件）。单配方计算直接用 [`jisuan`] 即可。
pub struct Calculator<'a> {
    catalog: &'a Catalog,
    index: ZhongjianIndex<'a>,
    jiage: &'a BTreeMap<String, Tong>,
    shoudong: &'a BTreeSet<String>,
    jingli_danjia: Tong,
    /// 中间产物单位成本 memo（请求内有效）
    memo: BTreeMap<String, ZizhiUnit>,
}

impl<'a> Calculator<'a> {
    pub fn new(
        catalog: &'a Catalog,
        jiage: &'a BTreeMap<String, Tong>,
        shoudong: &'a BTreeSet<String>,
        jingli_danjia: Tong,
    ) -> Self {
        Self {
            catalog,
            index: ZhongjianIndex::new(catalog),
            jiage,
            shoudong,
            jingli_danjia,
            memo: BTreeMap::new(),
        }
    }

    /// 中间产物索引（排行用来判断「中间产物配方」并跳过早算）。
    pub fn index(&self) -> &ZhongjianIndex<'a> {
        &self.index
    }

    /// 计算一个配方。
    ///
    /// `req` 中 `peifang_id` / `cishu` / `guadan_cishu` / `shouxufei` / `zhongjian_zizhi`
    /// 生效；价格上下文（`jiage` / `shoudong` / `jingli_danjia`）以构造参数为准。
    pub fn jisuan(&mut self, req: &JisuanRequest) -> Result<JisuanResult> {
        if req.cishu == 0 {
            return Err(JxcgError::Data("制作次数必须大于 0".into()));
        }
        if self.jingli_danjia < 0 {
            return Err(JxcgError::Money("精力单价不能为负".into()));
        }
        fee::validate_shouxufei(&req.shouxufei)?;
        // 只在实际取价处校验负数（保持 zhongjian_zizhi=false 与旧行为一致）

        let peifang = self
            .catalog
            .peifang
            .iter()
            .find(|p| p.id == req.peifang_id)
            .ok_or_else(|| JxcgError::Data(format!("配方不存在：{}", req.peifang_id)))?;
        let cishu = req.cishu as i128;
        let mut quejia: Vec<String> = Vec::new();

        // `zhongjian_zizhi=false` 时完全不展开、shoudong 也不生效（严格旧行为）
        let empty_shoudong = BTreeSet::new();
        let shoudong: &BTreeSet<String> = if req.zhongjian_zizhi {
            self.shoudong
        } else {
            &empty_shoudong
        };
        let price_ctx = PriceCtx {
            jiage: self.jiage,
            shoudong,
            jingli_danjia: self.jingli_danjia,
        };

        // ---- 材料（含中间产物自制展开） ----
        let mut cailiao = Vec::with_capacity(peifang.cailiao.len());
        let mut zhongjian_mx: Vec<ZhongjianMingxi> = Vec::new();
        let mut chengben_cailiao_i: i128 = 0;
        let mut chengben_zhongjian_i: i128 = 0;
        let mut jingli_zhongjian_i: i128 = 0;
        for item in &peifang.cailiao {
            let need = math::checked_mul(item.shuliang as i128, cishu, "材料需求量")?;
            // 中间产物默认自制展开；手填覆盖（shoudong）时按手填价采购不展开
            let should_expand = req.zhongjian_zizhi
                && self.index.is_zhongjian(&item.wupin_id)
                && !shoudong.contains(&item.wupin_id);
            let expanded = if should_expand {
                zhongjian::expand_unit(
                    &self.index,
                    &price_ctx,
                    &mut self.memo,
                    &item.wupin_id,
                    &mut quejia,
                )
            } else {
                None
            };

            let (danjia, laiyuan, is_zizhi) = match expanded {
                Some(unit) => {
                    let need_u32 = u32::try_from(need)
                        .map_err(|_| JxcgError::Data("中间产物顶层需求数量超出范围".into()))?;
                    let xiaojia_i = math::checked_mul(need, unit.danjia as i128, "材料小计")?;
                    let chain_jingli_i =
                        math::checked_mul(need, unit.unit_jingli as i128, "中间产物精力")?;
                    chengben_zhongjian_i =
                        math::checked_add(chengben_zhongjian_i, xiaojia_i, "中间产物成本")?;
                    jingli_zhongjian_i =
                        math::checked_add(jingli_zhongjian_i, chain_jingli_i, "中间产物精力")?;
                    zhongjian_mx.push(ZhongjianMingxi {
                        wupin_id: item.wupin_id.clone(),
                        mingcheng: wupin_mingcheng(self.catalog, &item.wupin_id),
                        peifang_id: unit.peifang_id.clone(),
                        shuliang: need_u32,
                        danjia: unit.danjia,
                        xiaojia: math::to_tong(xiaojia_i, "中间产物小计")?,
                        jingli: u32::try_from(chain_jingli_i)
                            .map_err(|_| JxcgError::Data("中间产物精力合计超出范围".into()))?,
                        cailiao: zhongjian_cailiao_mx(self.catalog, &unit, need)?,
                    });
                    (unit.danjia, "zizhi", true)
                }
                None => {
                    // 非中间产物 / 展开失败：按采购价（行情 → 商店 → 缺价）
                    let (price, source) = qu_danjia(
                        self.catalog,
                        self.jiage,
                        shoudong,
                        &item.wupin_id,
                        &mut quejia,
                    )?;
                    (price, source, false)
                }
            };

            let xiaojia_i = math::checked_mul(need, danjia as i128, "材料小计")?;
            chengben_cailiao_i = math::checked_add(chengben_cailiao_i, xiaojia_i, "材料成本")?;
            cailiao.push(CailiaoMingxi {
                wupin_id: item.wupin_id.clone(),
                mingcheng: wupin_mingcheng(self.catalog, &item.wupin_id),
                shuliang: item.shuliang,
                danjia,
                xiaojia: math::to_tong(xiaojia_i, "材料小计")?,
                laiyuan: laiyuan.to_string(),
                zhongjian: is_zizhi,
            });
        }

        // ---- 精力 / 制作费 / 保管费（仅最终配方自身） ----
        let jingli_i = math::checked_mul(peifang.jingli as i128, cishu, "精力消耗")?;
        let jingli =
            u32::try_from(jingli_i).map_err(|_| JxcgError::Data("精力消耗合计超出范围".into()))?;
        let chengben_jingli_i =
            math::checked_mul(jingli_i, self.jingli_danjia as i128, "精力成本")?;

        if peifang.zhizuofei < 0 {
            return Err(JxcgError::Data(format!(
                "配方制作费不能为负：{} = {}",
                peifang.id, peifang.zhizuofei
            )));
        }
        let zhizuofei_i = math::checked_mul(peifang.zhizuofei as i128, cishu, "制作费")?;

        // 挂单次数按「每批产出」计：批量 cishu 批 × 每批 guadan_cishu 次
        let baoguanfei_i = math::checked_mul(
            math::checked_mul(
                req.shouxufei.baoguanfei as i128,
                req.guadan_cishu as i128,
                "保管费",
            )?,
            cishu,
            "保管费",
        )?;

        let zong_i = math::checked_add(
            math::checked_add(
                math::checked_add(chengben_cailiao_i, chengben_jingli_i, "总成本")?,
                zhizuofei_i,
                "总成本",
            )?,
            baoguanfei_i,
            "总成本",
        )?;

        // ---- 期望产出与期望收入 ----
        let mut chuchan_mx = Vec::with_capacity(peifang.chuchan.len());
        let mut qiwang_num: i128 = 0; // 期望件数 = qiwang_num / QIWANG_DEN
        let mut shouru_num: i128 = 0; // 期望收入 = shouru_num / QIWANG_DEN（铜）
        for slot in &peifang.chuchan {
            if slot.min > slot.max {
                return Err(JxcgError::Data(format!(
                    "配方「{}」产物槽 min > max（{} > {}）",
                    peifang.id, slot.min, slot.max
                )));
            }
            let (danjia, _laiyuan) = qu_danjia(
                self.catalog,
                self.jiage,
                shoudong,
                &slot.wupin_id,
                &mut quejia,
            )?;
            let span = slot.min as i128 + slot.max as i128;
            let pieces_num = math::checked_mul(
                math::checked_mul(slot.quanzhong as i128, span, "期望产出")?,
                cishu,
                "期望产出",
            )?;
            qiwang_num = math::checked_add(qiwang_num, pieces_num, "期望产出")?;
            let slot_money = math::checked_mul(pieces_num, danjia as i128, "期望收入")?;
            shouru_num = math::checked_add(shouru_num, slot_money, "期望收入")?;
            chuchan_mx.push(ChuchanMingxi {
                wupin_id: slot.wupin_id.clone(),
                mingcheng: wupin_mingcheng(self.catalog, &slot.wupin_id),
                qiwang: pieces_num as f64 / QIWANG_DEN as f64,
                danjia,
            });
        }
        // 一次性 HALF_UP 到铜，避免逐槽 round 的累计误差
        let shouru = math::to_tong(math::round_div_half_up(shouru_num, QIWANG_DEN), "期望收入")?;

        // ---- 利润指标 ----
        let feiyong = fee::jisuan_shouxufei(shouru, &req.shouxufei);
        let jingshouru = shouru - feiyong;
        let zong_chengben = math::to_tong(zong_i, "总成本")?;
        let lirun = jingshouru
            .checked_sub(zong_chengben)
            .ok_or_else(|| JxcgError::Data("净利润溢出".into()))?;
        let lirunlv_bp = if zong_chengben > 0 {
            let bp = math::round_div_half_up_signed(lirun as i128 * 10_000, zong_chengben as i128);
            math::to_tong(bp, "利润率")?
        } else {
            0
        };
        let jingli_zhongjian = u32::try_from(jingli_zhongjian_i)
            .map_err(|_| JxcgError::Data("中间产物精力合计超出范围".into()))?;
        // 每精力利润 = 净利润 / (最终配方精力 + 中间产物精力)，整数截断（向 0 取整）
        let jingli_quanbu = jingli as i128 + jingli_zhongjian as i128;
        let mei_jingli = if jingli_quanbu > 0 {
            lirun / (jingli_quanbu as Tong)
        } else {
            lirun
        };

        // ---- 保本单价（按期望产出折算）----
        // 总成本已含保管费，先求「总期望收入」的最小保本额，再按期望件数折算单价。
        let baoben_total = fee::baobenjia(zong_chengben, &req.shouxufei, 0)?;
        let baoben_danjia = if qiwang_num > 0 {
            if baoben_total == 0 {
                0
            } else {
                // 求最小整数单价 p，使 round_half_up(p × qiwang) >= baoben_total：
                // 期望收入 = p × qiwang_num / QIWANG_DEN，HALF_UP 取整，
                // 即 2·p·qiwang_num >= (2·baoben_total − 1)·QIWANG_DEN。
                let num = math::checked_mul(2 * baoben_total as i128 - 1, QIWANG_DEN, "保本单价")?;
                let den = 2 * qiwang_num;
                math::to_tong(math::ceil_div(num, den), "保本单价")?
            }
        } else {
            // 无期望产出（数据异常）：退化为总保本收入
            baoben_total
        };

        // 缺价去重（排序后去重，结果稳定，便于前端提示与测试）
        quejia.sort();
        quejia.dedup();

        Ok(JisuanResult {
            peifang_id: peifang.id.clone(),
            peifang_mingcheng: peifang.mingcheng.clone(),
            chengben_cailiao: math::to_tong(chengben_cailiao_i, "材料成本")?,
            chengben_zhongjian: math::to_tong(chengben_zhongjian_i, "中间产物成本")?,
            chengben_jingli: math::to_tong(chengben_jingli_i, "精力成本")?,
            zhizuofei: math::to_tong(zhizuofei_i, "制作费")?,
            baoguanfei: math::to_tong(baoguanfei_i, "保管费")?,
            zong_chengben,
            shouru,
            jingshouru,
            lirun,
            lirunlv_bp,
            mei_jingli,
            baobenjia: baoben_danjia,
            jingli,
            jingli_zhongjian,
            chuchan_qiwang: qiwang_num as f64 / QIWANG_DEN as f64,
            cailiao,
            zhongjian: zhongjian_mx,
            chuchan: chuchan_mx,
            quejia,
        })
    }
}

/// 中间产物明细里的「一层原料」：按顶层需求数量折算（展示用，数量 HALF_UP 到整数）。
fn zhongjian_cailiao_mx(
    catalog: &Catalog,
    unit: &ZizhiUnit,
    need: i128,
) -> Result<Vec<CailiaoMingxi>> {
    let mut rows = Vec::with_capacity(unit.cailiao.len());
    for row in &unit.cailiao {
        // 顶层需求下的期望用量 = 每批用量 × 顶层需求 / 每批期望产出
        let num = math::checked_mul(
            math::checked_mul(row.shuliang as i128, need, "中间产物原料数量")?,
            QIWANG_DEN,
            "中间产物原料数量",
        )?;
        let qty_i = math::round_div_half_up(num, unit.q_num);
        let qty =
            u32::try_from(qty_i).map_err(|_| JxcgError::Data("中间产物原料数量超出范围".into()))?;
        let xiaojia_i = math::checked_mul(row.danjia as i128, qty as i128, "中间产物原料小计")?;
        rows.push(CailiaoMingxi {
            wupin_id: row.wupin_id.clone(),
            mingcheng: wupin_mingcheng(catalog, &row.wupin_id),
            shuliang: qty,
            danjia: row.danjia,
            xiaojia: math::to_tong(xiaojia_i, "中间产物原料小计")?,
            laiyuan: row.laiyuan.clone(),
            zhongjian: row.zhongjian,
        });
    }
    Ok(rows)
}

/// 计算一个配方的成本与利润（单次调用；内部建索引 + memo）。
///
/// `cishu` = 制作次数（必须 >= 1）；`guadan_cishu` = 每批产出的挂单次数（0 表示不计保管费）。
pub fn jisuan(catalog: &Catalog, req: &JisuanRequest) -> Result<JisuanResult> {
    let mut calculator = Calculator::new(catalog, &req.jiage, &req.shoudong, req.jingli_danjia);
    calculator.jisuan(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jxcg_api::{Banben, Cailiao, Jiyi, Peifang, Pinlei, Quzheng, Shouxufei};

    fn wupin(id: &str, mingcheng: &str, shop: Option<Tong>) -> Wupin {
        Wupin {
            id: id.into(),
            mingcheng: mingcheng.into(),
            pinlei: Pinlei::Cailiao,
            dui: None,
            shangdianjia: shop,
            laiyuan: None,
        }
    }

    fn peifang(id: &str, jingli: u32, cailiao: Vec<Cailiao>, chuchan: Vec<Chuchan>) -> Peifang {
        Peifang {
            id: id.into(),
            mingcheng: format!("配方{id}"),
            jiyi: Jiyi::Pengren,
            dengji: 10,
            jingli,
            zhizuofei: 0,
            cailiao,
            chuchan,
            laiyuan: None,
        }
    }

    fn cailiao(wupin_id: &str, shuliang: u32) -> Cailiao {
        Cailiao {
            wupin_id: wupin_id.into(),
            shuliang,
        }
    }

    fn chuchan(wupin_id: &str, min: u32, max: u32, quanzhong: u32) -> Chuchan {
        Chuchan {
            wupin_id: wupin_id.into(),
            min,
            max,
            quanzhong,
        }
    }

    /// 基础目录：材料 5_1（商店价 100）、成品 5_2 / 5_3（无商店价）
    fn catalog(peifang_list: Vec<Peifang>) -> Catalog {
        Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("5_1", "材料", Some(100)),
                wupin("5_2", "成品", None),
                wupin("5_3", "精制品", None),
                wupin("5_9", "无商店价材料", None),
            ],
            peifang: peifang_list,
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        }
    }

    fn shouxufei(baoguanfei: Tong) -> Shouxufei {
        Shouxufei {
            shuilv_bp: 500,
            quzheng: Quzheng::HalfUp,
            baoguanfei,
        }
    }

    fn request(peifang_id: &str) -> JisuanRequest {
        JisuanRequest {
            peifang_id: peifang_id.into(),
            cishu: 1,
            guadan_cishu: 1,
            jingli_danjia: 0,
            shouxufei: shouxufei(0),
            jiage: BTreeMap::new(),
            jiyi_dengji: 0,
            zhongjian_zizhi: true,
            shoudong: BTreeSet::new(),
        }
    }

    #[test]
    fn qiwang_weighted() {
        // 普通 1006633（96%）+ 精制 41943（4%），数量 2~3 → 期望 2.5 件
        let slots = vec![chuchan("a", 2, 3, 1_006_633), chuchan("b", 2, 3, 41_943)];
        assert!((chuchan_qiwang(&slots) - 2.5).abs() < 1e-9);
        // 零权重槽不贡献
        let with_zero = vec![
            chuchan("a", 2, 3, QUANZHONG_FENMU),
            chuchan("b", 100, 100, 0),
        ];
        assert!((chuchan_qiwang(&with_zero) - 2.5).abs() < 1e-9);
        // 空产出 / 0 权重
        assert_eq!(chuchan_qiwang(&[]), 0.0);
        assert_eq!(chuchan_qiwang(&[chuchan("a", 1, 1, 0)]), 0.0);
    }

    #[test]
    fn basic_profit() {
        let cat = catalog(vec![peifang(
            "p1",
            20,
            vec![cailiao("5_1", 2)],
            vec![chuchan("5_2", 1, 1, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.jingli_danjia = 10;
        req.jiage = BTreeMap::from([("5_1".into(), 200), ("5_2".into(), 1_000)]);
        let r = jisuan(&cat, &req).unwrap();
        // 材料 2×200 = 400；精力 20×10 = 200；总成本 600
        assert_eq!(r.chengben_cailiao, 400);
        assert_eq!(r.chengben_jingli, 200);
        assert_eq!(r.zong_chengben, 600);
        // 收入 1000，手续费 50，税后 950；净利润 350
        assert_eq!(r.shouru, 1_000);
        assert_eq!(r.jingshouru, 950);
        assert_eq!(r.lirun, 350);
        assert_eq!(r.lirunlv_bp, 5_833); // 350/600 = 58.333% → 5833 万分比
        assert_eq!(r.mei_jingli, 17); // 350/20 = 17.5 → 截断 17
        assert_eq!(r.jingli, 20);
        assert!((r.chuchan_qiwang - 1.0).abs() < 1e-9);
        // 保本总价：5% 后覆盖 600 的最小收入 632；单价 = 632
        assert_eq!(r.baobenjia, 632);
        assert!(r.quejia.is_empty());
        // 材料明细
        assert_eq!(r.cailiao.len(), 1);
        assert_eq!(r.cailiao[0].danjia, 200);
        assert_eq!(r.cailiao[0].xiaojia, 400);
        assert_eq!(r.cailiao[0].laiyuan, "hangqing");
    }

    #[test]
    fn price_priority_and_quejia() {
        // 5_1 有行情 → 行情优先；5_9 无行情无商店 → 缺价；成品没有价格 → 缺价
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![cailiao("5_1", 1), cailiao("5_9", 2), cailiao("5_9", 3)],
            vec![chuchan("5_2", 1, 1, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_1".into(), 500)]);
        let r = jisuan(&cat, &req).unwrap();
        // 行情价 500 覆盖商店价 100
        assert_eq!(r.cailiao[0].danjia, 500);
        assert_eq!(r.cailiao[0].laiyuan, "hangqing");
        // 缺价材料按 0 计
        assert_eq!(r.cailiao[1].danjia, 0);
        assert_eq!(r.cailiao[2].danjia, 0);
        assert_eq!(r.cailiao[1].laiyuan, "quejia");
        assert_eq!(r.chengben_cailiao, 500);
        // 缺价去重且稳定排序（5_2 成品缺价、5_9 材料缺价）
        assert_eq!(r.quejia, vec!["5_2".to_string(), "5_9".to_string()]);
        assert_eq!(r.chuchan[0].danjia, 0);

        // 只用商店价时来源标记 shangdian
        let mut req2 = request("p1");
        req2.peifang_id = "p1".into();
        let cat2 = catalog(vec![peifang(
            "p2",
            0,
            vec![cailiao("5_1", 3)],
            vec![chuchan("5_2", 1, 1, QUANZHONG_FENMU)],
        )]);
        req2.peifang_id = "p2".into();
        req2.jiage = BTreeMap::new();
        let r2 = jisuan(&cat2, &req2).unwrap();
        assert_eq!(r2.cailiao[0].danjia, 100);
        assert_eq!(r2.cailiao[0].laiyuan, "shangdian");
        assert_eq!(r2.chengben_cailiao, 300);
    }

    #[test]
    fn expected_revenue_rounds_once() {
        // 两个槽各 0.5 件、单价 1 铜：期望收入 = 1 铜。
        // 若逐槽 HALF_UP 会得到 1+1=2 铜，这里钉死「最后一次性取整」。
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![],
            vec![
                chuchan("5_2", 1, 1, QUANZHONG_FENMU / 2),
                chuchan("5_3", 1, 1, QUANZHONG_FENMU / 2),
            ],
        )]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_2".into(), 1), ("5_3".into(), 1)]);
        let r = jisuan(&cat, &req).unwrap();
        assert!((r.chuchan_qiwang - 1.0).abs() < 1e-9);
        assert_eq!(r.shouru, 1);
        // HALF_UP：0.5 铜期望收入也进到 1 铜
        assert_eq!(r.lirun, 1);
        assert_eq!(r.lirunlv_bp, 0); // 总成本为 0 时利润率记 0
    }

    #[test]
    fn batch_scales_linearly() {
        let cat = catalog(vec![peifang(
            "p1",
            10,
            vec![cailiao("5_1", 2)],
            vec![chuchan("5_2", 2, 2, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.jingli_danjia = 5;
        req.jiage = BTreeMap::from([("5_1".into(), 100), ("5_2".into(), 1_000)]);
        req.shouxufei = shouxufei(8_000);
        req.guadan_cishu = 2;
        req.cishu = 3;
        let r = jisuan(&cat, &req).unwrap();
        // 材料 2×100×3 = 600；精力 10×3×5 = 150；保管费 8000×2×3 = 48000
        assert_eq!(r.chengben_cailiao, 600);
        assert_eq!(r.chengben_jingli, 150);
        assert_eq!(r.baoguanfei, 48_000);
        assert_eq!(r.zhizuofei, 0);
        assert_eq!(r.zong_chengben, 600 + 150 + 48_000);
        // 产出 2 件×3 批 = 6 件，收入 6000，手续费 300，税后 5700
        assert!((r.chuchan_qiwang - 6.0).abs() < 1e-9);
        assert_eq!(r.shouru, 6_000);
        assert_eq!(r.jingshouru, 5_700);
        assert_eq!(r.lirun, 5_700 - 48_750);
        assert_eq!(r.jingli, 30);
        // 单批结果 ×3 与批量结果一致（线性）
        let mut one = req.clone();
        one.cishu = 1;
        let r1 = jisuan(&cat, &one).unwrap();
        assert_eq!(r.chengben_cailiao, r1.chengben_cailiao * 3);
        assert_eq!(r.chengben_jingli, r1.chengben_jingli * 3);
        assert_eq!(r.baoguanfei, r1.baoguanfei * 3);
        assert_eq!(r.shouru, r1.shouru * 3);
        assert_eq!(r.zong_chengben, r1.zong_chengben * 3);
    }

    #[test]
    fn custody_can_be_zero() {
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![],
            vec![chuchan("5_2", 1, 1, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_2".into(), 1_000)]);
        req.shouxufei = shouxufei(8_000);
        req.guadan_cishu = 0;
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.baoguanfei, 0);
        assert_eq!(r.zong_chengben, 0);
        assert_eq!(r.lirun, 950);
        // 挂一单时扣 8000
        req.guadan_cishu = 1;
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.baoguanfei, 8_000);
        assert_eq!(r.lirun, 950 - 8_000);
    }

    #[test]
    fn lirunlv_rounding() {
        // 材料成本 3、税率 0、单件产出 → 用售价控制利润
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![cailiao("5_1", 1)],
            vec![chuchan("5_2", 1, 1, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_1".into(), 3)]);
        for (price, expect_bp) in [
            (5, 6_667),  // 2/3 = 66.67%
            (4, 3_333),  // 1/3 = 33.33%
            (2, -3_333), // -1/3 = -33.33%（负数 HALF_UP 远离 0）
            (0, -10_000),
        ] {
            req.jiage.insert("5_2".into(), price);
            let r = jisuan(&cat, &req).unwrap();
            assert_eq!(r.lirun, price - 3, "price={price}");
            assert_eq!(r.lirunlv_bp, expect_bp, "price={price}");
        }
        // 负利润但成本 8、利润 -1 → -1250 整
        req.jiage.insert("5_1".into(), 8);
        req.jiage.insert("5_2".into(), 7);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.lirun, -1);
        assert_eq!(r.lirunlv_bp, -1_250);
        // 每精力利润：-7 利润 / 3 精力 = -2（截断向 0）
        let cat2 = catalog(vec![peifang(
            "p2",
            3,
            vec![],
            vec![chuchan("5_2", 1, 1, QUANZHONG_FENMU)],
        )]);
        let mut req2 = request("p2");
        req2.jiage = BTreeMap::from([("5_2".into(), 4)]);
        req2.jingli_danjia = 0;
        let r2 = jisuan(&cat2, &req2).unwrap();
        // 收入 4、手续费 0（HALF_UP 0.2 → 0）、利润 4；改成亏损场景：精力单价
        assert_eq!(r2.lirun, 4);
        assert_eq!(r2.mei_jingli, 1);
        req2.jingli_danjia = 4;
        let r2 = jisuan(&cat2, &req2).unwrap();
        // 精力成本 3×4=12 → 利润 4-12 = -8；-8/3 = -2（截断）
        assert_eq!(r2.lirun, -8);
        assert_eq!(r2.mei_jingli, -2);
    }

    #[test]
    fn baoben_multiple_pieces() {
        // 成本 1000、每批期望产出 2 件；5% 手续费。
        // 保本总收入 1053 → 最小单价 527（527×2=1054，税后 1001 >= 1000；526 时 999 < 1000）。
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![cailiao("5_1", 1)],
            vec![chuchan("5_2", 2, 2, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_1".into(), 1_000)]);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.zong_chengben, 1_000);
        assert_eq!(r.baobenjia, 527);
        // 527 保本，526 不够
        let mut check = req.clone();
        check.jiage.insert("5_2".into(), 527);
        let ok = jisuan(&cat, &check).unwrap();
        assert!(ok.lirun >= 0);
        check.jiage.insert("5_2".into(), 526);
        let fail = jisuan(&cat, &check).unwrap();
        assert!(fail.lirun < 0);
    }

    #[test]
    fn baoben_fractional_output() {
        // 期望产出 2.5 件/次，成本 1000：保本单价应等于「暴力扫描出的最小可行单价」。
        // 421×2.5 = 1052.5 → HALF_UP 1053 → 手续费 53 → 税后 1000，刚好保本；
        // 420 时收入 1050 → 手续费 53 → 997 < 1000，不保本。
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![cailiao("5_1", 1)],
            vec![chuchan("5_2", 2, 3, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_1".into(), 1_000)]);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.baobenjia, 421);
        // 暴力扫描校验最小性：p 是第一个 lirun >= 0 的整数单价
        let mut first_ok = None;
        for price in 0..=600 {
            let mut probe = req.clone();
            probe.jiage.insert("5_2".into(), price);
            if jisuan(&cat, &probe).unwrap().lirun >= 0 {
                first_ok = Some(price);
                break;
            }
        }
        assert_eq!(first_ok, Some(r.baobenjia));
    }

    #[test]
    fn no_output() {
        // 无产出：期望收入 0，利润为负，保本单价退化为总保本收入
        let cat = catalog(vec![peifang("p1", 10, vec![cailiao("5_1", 1)], vec![])]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_1".into(), 100)]);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.chuchan_qiwang, 0.0);
        assert_eq!(r.shouru, 0);
        assert_eq!(r.lirun, -100);
        assert_eq!(r.baobenjia, 105); // 100 → 5% 后覆盖 100 的最小整数收入 105
    }

    #[test]
    fn partial_weights_not_normalized() {
        // 两个槽权重只占 50%：期望产出 1 件（不重新归一化）
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![],
            vec![
                chuchan("5_2", 2, 2, QUANZHONG_FENMU / 4),
                chuchan("5_3", 2, 2, QUANZHONG_FENMU / 4),
            ],
        )]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_2".into(), 10), ("5_3".into(), 10)]);
        let r = jisuan(&cat, &req).unwrap();
        assert!((r.chuchan_qiwang - 1.0).abs() < 1e-9);
        assert_eq!(r.shouru, 10);
    }

    #[test]
    fn rounding_of_expected_revenue() {
        // 权重满、数量 1~2（平均 1.5）× 单价 1 = 1.5 铜 → HALF_UP 2 铜
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![],
            vec![chuchan("5_2", 1, 2, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_2".into(), 1)]);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.shouru, 2);
        req.jiage.insert("5_2".into(), 0);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.shouru, 0);
    }

    #[test]
    fn errors() {
        let cat = catalog(vec![peifang(
            "p1",
            10,
            vec![cailiao("5_1", 1)],
            vec![chuchan("5_2", 1, 1, QUANZHONG_FENMU)],
        )]);
        let mut req = request("p1");
        req.cishu = 0;
        assert!(jisuan(&cat, &req).is_err());
        let mut req = request("p1");
        req.jingli_danjia = -1;
        assert!(jisuan(&cat, &req).is_err());
        let mut req = request("p1");
        req.shouxufei.shuilv_bp = 10_000;
        assert!(jisuan(&cat, &req).is_err());
        let mut req = request("p1");
        req.jiage = BTreeMap::from([("5_1".into(), -5)]);
        assert!(jisuan(&cat, &req).is_err());
        let req = request("nope");
        assert!(jisuan(&cat, &req).is_err());
        // 材料单价 i64::MAX × 数量 2 → 溢出报错而不是 panic
        let mut req = request("p1");
        req.cishu = 2;
        req.jiage = BTreeMap::from([("5_1".into(), i64::MAX)]);
        assert!(jisuan(&cat, &req).is_err());
    }

    #[test]
    fn min_max_invalid() {
        let cat = catalog(vec![peifang(
            "p1",
            0,
            vec![],
            vec![chuchan("5_2", 3, 1, QUANZHONG_FENMU)],
        )]);
        let req = request("p1");
        assert!(jisuan(&cat, &req).is_err());
    }

    // ========================================================================
    // 中间产物自制成本展开（PLAN.md §6.3.1）
    // ========================================================================

    /// 双级链路目录：
    /// - 采购原料 raw_a（商店价 100）、raw_b（商店价 150）；
    /// - 中间产物 mid：配方 = 2×raw_a + 1×raw_b + 精力 10 + 制作费 500，
    ///   每次期望产出 2.5 件（min2 max3 权重满）→ 单位成本 = 1000 / 2.5 = 400；
    /// - 最终配方 final_recipe：3×mid + 自身精力 20，产出成品 out 1 件。
    fn zhongjian_catalog() -> Catalog {
        let mut mid = peifang(
            "mid_recipe",
            10,
            vec![cailiao("raw_a", 2), cailiao("raw_b", 1)],
            vec![chuchan("mid", 2, 3, QUANZHONG_FENMU)],
        );
        mid.zhizuofei = 500;
        Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("raw_a", "原料A", Some(100)),
                wupin("raw_b", "原料B", Some(150)),
                wupin("mid", "中间产物", None),
                wupin("out", "成品", None),
            ],
            peifang: vec![
                mid,
                peifang(
                    "final_recipe",
                    20,
                    vec![cailiao("mid", 3)],
                    vec![chuchan("out", 1, 1, QUANZHONG_FENMU)],
                ),
            ],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        }
    }

    fn zhongjian_req() -> JisuanRequest {
        let mut req = request("final_recipe");
        req.jingli_danjia = 10;
        req.shouxufei = shouxufei(8_000);
        req.jiage = BTreeMap::from([
            ("raw_a".to_string(), 100),
            ("raw_b".to_string(), 200),
            ("mid".to_string(), 999), // 行情价：中间产物默认自制，不用它
            ("out".to_string(), 5_000),
        ]);
        req
    }

    #[test]
    fn zhongjian_expand_two_levels() {
        let cat = zhongjian_catalog();
        let req = zhongjian_req();
        let r = jisuan(&cat, &req).unwrap();

        // 中间产物单位成本：每批 (2×100 + 1×200 + 10×10 + 500) = 1000，期望 2.5 件 → 400
        assert_eq!(r.cailiao.len(), 1);
        assert_eq!(r.cailiao[0].danjia, 400);
        assert_eq!(r.cailiao[0].xiaojia, 1_200);
        assert!(r.cailiao[0].zhongjian);
        assert_eq!(r.cailiao[0].laiyuan, "zizhi");
        assert_eq!(r.chengben_cailiao, 1_200);
        // 中间产物成本展示值已含在材料成本里
        assert_eq!(r.chengben_zhongjian, 1_200);
        // 最终配方自身精力 20×10=200；中间产物精力 3×(10/2.5=4)=12
        assert_eq!(r.jingli, 20);
        assert_eq!(r.chengben_jingli, 200);
        assert_eq!(r.jingli_zhongjian, 12);
        assert_eq!(r.zhizuofei, 0);
        assert_eq!(r.baoguanfei, 8_000);
        // zong 必须严格等于各项之和
        assert_eq!(
            r.zong_chengben,
            r.chengben_cailiao + r.chengben_jingli + r.zhizuofei + r.baoguanfei
        );
        assert_eq!(r.zong_chengben, 9_400);
        // 收入 5000 − 手续费 250 = 4750；利润 4750 − 9400 = −4650
        assert_eq!(r.shouru, 5_000);
        assert_eq!(r.jingshouru, 4_750);
        assert_eq!(r.lirun, -4_650);
        // 每精力利润 = −4650 / (20 + 12) = −145（截断）
        assert_eq!(r.mei_jingli, -145);

        assert_eq!(r.zhongjian.len(), 1);
        let zj = &r.zhongjian[0];
        assert_eq!(zj.wupin_id, "mid");
        assert_eq!(zj.mingcheng, "中间产物");
        assert_eq!(zj.peifang_id, "mid_recipe");
        assert_eq!(zj.shuliang, 3);
        assert_eq!(zj.danjia, 400);
        assert_eq!(zj.xiaojia, 1_200);
        assert_eq!(zj.jingli, 12);
        // 一层原料按顶层需求折算：raw_a 2×3/2.5 = 2.4 → 2；raw_b 1×3/2.5 = 1.2 → 1
        assert_eq!(zj.cailiao.len(), 2);
        assert_eq!(zj.cailiao[0].wupin_id, "raw_a");
        assert_eq!(zj.cailiao[0].shuliang, 2);
        assert_eq!(zj.cailiao[0].danjia, 100);
        assert_eq!(zj.cailiao[0].laiyuan, "hangqing");
        assert!(!zj.cailiao[0].zhongjian);
        assert_eq!(zj.cailiao[1].wupin_id, "raw_b");
        assert_eq!(zj.cailiao[1].shuliang, 1);
        assert_eq!(zj.cailiao[1].danjia, 200);
        assert!(r.quejia.is_empty());
    }

    #[test]
    fn zhongjian_three_levels() {
        // raw(100) → mid1（2×raw + 精力 2，产出 2 件）→ mid2（3×mid1 + 精力 4，产出 1 件）→ final（2×mid2）
        let mut mid1 = peifang(
            "mid1_recipe",
            2,
            vec![cailiao("raw", 2)],
            vec![chuchan("mid1", 2, 2, QUANZHONG_FENMU)],
        );
        mid1.zhizuofei = 0;
        let mut mid2 = peifang(
            "mid2_recipe",
            4,
            vec![cailiao("mid1", 3)],
            vec![chuchan("mid2", 1, 1, QUANZHONG_FENMU)],
        );
        mid2.zhizuofei = 0;
        let cat = Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("raw", "原料", Some(100)),
                wupin("mid1", "中间1", None),
                wupin("mid2", "中间2", None),
                wupin("out", "成品", None),
            ],
            peifang: vec![
                mid1,
                mid2,
                peifang(
                    "final_recipe",
                    0,
                    vec![cailiao("mid2", 2)],
                    vec![chuchan("out", 1, 1, QUANZHONG_FENMU)],
                ),
            ],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        };
        let mut req = request("final_recipe");
        req.jingli_danjia = 10;
        req.jiage = BTreeMap::from([("raw".to_string(), 100), ("out".to_string(), 1_000)]);
        let r = jisuan(&cat, &req).unwrap();
        // mid1：每批 2×100 + 2×10 = 220，产出 2 → 单位 110，单位精力 1
        // mid2：每批 3×110 + 4×10 = 370，产出 1 → 单位 370，单位精力 3×1+4 = 7
        // final：2×370 = 740；中间精力 2×7 = 14
        assert_eq!(r.chengben_cailiao, 740);
        assert_eq!(r.jingli_zhongjian, 14);
        assert_eq!(r.zhongjian[0].danjia, 370);
        assert_eq!(r.zhongjian[0].jingli, 14);
        // 收入 1000 − 50 = 950；利润 950 − 740 = 210；每精力 210/14 = 15
        assert_eq!(r.lirun, 210);
        assert_eq!(r.mei_jingli, 15);
        assert!(r.quejia.is_empty());
    }

    #[test]
    fn zhongjian_cycle_fallback() {
        // A 需要 B、B 需要 A 的环：环处回退采购价（行情价 300），不递归爆栈
        let cat = Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("mid_a", "A", None),
                wupin("mid_b", "B", None),
                wupin("out", "成品", None),
            ],
            peifang: vec![
                peifang(
                    "a_recipe",
                    0,
                    vec![cailiao("mid_b", 1)],
                    vec![chuchan("mid_a", 1, 1, QUANZHONG_FENMU)],
                ),
                peifang(
                    "b_recipe",
                    0,
                    vec![cailiao("mid_a", 1)],
                    vec![chuchan("mid_b", 1, 1, QUANZHONG_FENMU)],
                ),
                peifang(
                    "final_recipe",
                    0,
                    vec![cailiao("mid_a", 1)],
                    vec![chuchan("out", 1, 1, QUANZHONG_FENMU)],
                ),
            ],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        };
        let mut req = request("final_recipe");
        req.jiage = BTreeMap::from([("mid_a".to_string(), 300), ("out".to_string(), 1_000)]);
        let r = jisuan(&cat, &req).unwrap();
        // 环处在 B 的材料 A 上回退行情价 300 → A 单位成本 300（回退后不再展开）
        assert_eq!(r.cailiao[0].danjia, 300);
        assert!(r.cailiao[0].zhongjian);
        assert_eq!(r.zhongjian[0].peifang_id, "a_recipe");
        // 无行情价时回退缺价（0）并汇总 quejia，同样不崩溃
        let mut req2 = request("final_recipe");
        req2.jiage = BTreeMap::from([("out".to_string(), 1_000)]);
        let r2 = jisuan(&cat, &req2).unwrap();
        assert_eq!(r2.cailiao[0].danjia, 0);
        assert!(r2.quejia.contains(&"mid_a".to_string()));
    }

    #[test]
    fn zhongjian_zero_expected_output_fallback() {
        // 产出权重为 0 → 期望产出 0 → 不可展开，回退行情价
        let mut bad = peifang(
            "mid_recipe",
            0,
            vec![cailiao("raw", 1)],
            vec![chuchan("mid", 1, 1, 0)],
        );
        bad.zhizuofei = 0;
        let cat = Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("raw", "原料", Some(100)),
                wupin("mid", "中间产物", None),
                wupin("out", "成品", None),
            ],
            peifang: vec![
                bad,
                peifang(
                    "final_recipe",
                    0,
                    vec![cailiao("mid", 2)],
                    vec![chuchan("out", 1, 1, QUANZHONG_FENMU)],
                ),
            ],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        };
        let mut req = request("final_recipe");
        req.jiage = BTreeMap::from([("mid".to_string(), 500), ("out".to_string(), 1_000)]);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.cailiao[0].danjia, 500);
        assert!(!r.cailiao[0].zhongjian);
        assert_eq!(r.cailiao[0].laiyuan, "hangqing");
        assert_eq!(r.chengben_zhongjian, 0);
        assert!(r.zhongjian.is_empty());
        // 期望产出 > 0 但单位成本折算的 HALF_UP 边界：1 铜/2.5 件 = 0.4 → 0 铜
    }

    #[test]
    fn zhongjian_manual_override() {
        let cat = zhongjian_catalog();
        let mut req = zhongjian_req();
        req.shoudong = BTreeSet::from(["mid".to_string()]);
        let r = jisuan(&cat, &req).unwrap();
        // 手填覆盖：按 jiage=999? 手填价应取 jiage 里的值（这里行情 999 当作手填价）
        assert_eq!(r.cailiao[0].danjia, 999);
        assert_eq!(r.cailiao[0].laiyuan, "shoudong");
        assert!(!r.cailiao[0].zhongjian);
        assert_eq!(r.chengben_cailiao, 2_997);
        assert_eq!(r.chengben_zhongjian, 0);
        assert_eq!(r.jingli_zhongjian, 0);
        assert!(r.zhongjian.is_empty());

        // 更深一层采购原料手填：raw_a 手填 333 → mid 单位成本 = (2×333+200+100+500)/2.5 = 586.4 → 586
        let mut req2 = zhongjian_req();
        req2.jiage.insert("raw_a".to_string(), 333);
        req2.shoudong = BTreeSet::from(["raw_a".to_string()]);
        let r2 = jisuan(&cat, &req2).unwrap();
        assert_eq!(r2.cailiao[0].danjia, 586);
        assert_eq!(r2.zhongjian[0].cailiao[0].laiyuan, "shoudong");
        assert_eq!(r2.zhongjian[0].cailiao[0].danjia, 333);
    }

    #[test]
    fn zhongjian_disabled_matches_old_behavior() {
        let cat = zhongjian_catalog();
        let mut req = zhongjian_req();
        req.zhongjian_zizhi = false;
        let r = jisuan(&cat, &req).unwrap();
        // 旧行为：中间产物按行情价 999 计入材料成本
        assert_eq!(r.cailiao[0].danjia, 999);
        assert_eq!(r.cailiao[0].laiyuan, "hangqing");
        assert!(!r.cailiao[0].zhongjian);
        assert_eq!(r.chengben_cailiao, 2_997);
        assert_eq!(r.chengben_zhongjian, 0);
        assert_eq!(r.jingli_zhongjian, 0);
        assert!(r.zhongjian.is_empty());
        assert_eq!(r.zong_chengben, 2_997 + 200 + 8_000);
        assert_eq!(r.lirun, 4_750 - 11_197);
        // 每精力利润退回只按最终精力：−6447/20 = −322（截断）
        assert_eq!(r.mei_jingli, -322);
        // shoudong 在关闭时完全不生效（结果逐字段一致）
        let mut req2 = req.clone();
        req2.shoudong = BTreeSet::from(["mid".to_string(), "raw_a".to_string()]);
        assert_eq!(jisuan(&cat, &req2).unwrap(), r);
    }

    #[test]
    fn zhongjian_producer_selection() {
        // 同物品多配方：取 (dengji, id) 最小者。
        // p_high（dengji 30，单位成本 100）vs p_low（dengji 10，单位成本 300）→ 选 p_low；
        // 同 dengji 时按 id 升序。
        let make = |id: &str, dengji: u32, raw: &str| {
            let mut p = peifang(
                id,
                dengji,
                vec![cailiao(raw, 1)],
                vec![chuchan("mid", 1, 1, QUANZHONG_FENMU)],
            );
            p.dengji = dengji;
            p
        };
        let cat = Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("raw_cheap", "便宜原料", Some(100)),
                wupin("raw_expensive", "贵原料", Some(300)),
                wupin("mid", "中间产物", None),
                wupin("out", "成品", None),
            ],
            peifang: vec![
                make("p_high", 30, "raw_cheap"),
                make("p_low", 10, "raw_expensive"),
                peifang(
                    "final_recipe",
                    0,
                    vec![cailiao("mid", 1)],
                    vec![chuchan("out", 1, 1, QUANZHONG_FENMU)],
                ),
            ],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        };
        let mut req = request("final_recipe");
        req.jiage = BTreeMap::from([("out".to_string(), 1_000)]);
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.cailiao[0].danjia, 300);
        assert_eq!(r.zhongjian[0].peifang_id, "p_low");

        // 同等级按 id 升序：p_b 在前（成本 100），p_a 成本 300 → 选 p_a（id 小）
        let cat2 = Catalog {
            banben: Banben::default(),
            wupin: cat.wupin.clone(),
            peifang: vec![
                make("p_b", 10, "raw_cheap"),
                make("p_a", 10, "raw_expensive"),
                cat.peifang[2].clone(),
            ],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        };
        let r2 = jisuan(&cat2, &req).unwrap();
        assert_eq!(r2.zhongjian[0].peifang_id, "p_a");
        assert_eq!(r2.cailiao[0].danjia, 300);
    }

    #[test]
    fn zhongjian_quejia_propagation() {
        // 中间产物的采购原料缺价：按 0 计入，但 quejia 要冒泡到顶层结果
        let mut mid = peifang(
            "mid_recipe",
            0,
            vec![cailiao("missing_raw", 2)],
            vec![chuchan("mid", 1, 1, QUANZHONG_FENMU)],
        );
        mid.zhizuofei = 0;
        let cat = Catalog {
            banben: Banben::default(),
            wupin: vec![
                wupin("missing_raw", "缺价原料", None),
                wupin("mid", "中间产物", None),
                wupin("out", "成品", None),
            ],
            peifang: vec![
                mid,
                peifang(
                    "final_recipe",
                    0,
                    vec![cailiao("mid", 1)],
                    vec![chuchan("out", 1, 1, QUANZHONG_FENMU)],
                ),
            ],
            wuxingshi: vec![],
            fuwuqi: vec![],
            zhiji: vec![],
        };
        // 成品也缺价 → quejia 同时包含采购缺价与产出缺价，排序去重
        let req = request("final_recipe");
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.cailiao[0].danjia, 0);
        assert_eq!(r.zhongjian[0].cailiao[0].laiyuan, "quejia");
        assert_eq!(r.quejia, vec!["missing_raw".to_string(), "out".to_string()]);
    }

    #[test]
    fn zhongjian_batch_scaling() {
        let cat = zhongjian_catalog();
        let mut req = zhongjian_req();
        req.cishu = 2;
        req.guadan_cishu = 2;
        let r = jisuan(&cat, &req).unwrap();
        assert_eq!(r.cailiao[0].shuliang, 3);
        assert_eq!(r.cailiao[0].xiaojia, 6 * 400);
        assert_eq!(r.chengben_cailiao, 2_400);
        assert_eq!(r.zhongjian[0].shuliang, 6);
        assert_eq!(r.zhongjian[0].xiaojia, 2_400);
        assert_eq!(r.zhongjian[0].jingli, 24);
        assert_eq!(r.jingli, 40);
        assert_eq!(r.jingli_zhongjian, 24);
        assert_eq!(r.chengben_jingli, 400);
        assert_eq!(r.baoguanfei, 32_000); // 8000 × 2 挂单 × 2 批
        assert_eq!(
            r.zong_chengben,
            r.chengben_cailiao + r.chengben_jingli + r.zhizuofei + r.baoguanfei
        );
    }
}
