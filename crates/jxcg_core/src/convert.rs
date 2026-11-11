//! 五行石精炼（转换配方）计算。
//!
//! 模型：每次投入 `xuyao` 个低阶石 + 精力 + 金币，成功率 `p` → 期望需要 1/p 次投入。
//! 默认规则来自社区整理（见 data/wuxingshi.toml），可编辑、待实机校准。

use std::collections::BTreeMap;

use crate::{JxcgError, Result, fee};
use jxcg_api::{Catalog, Tong, ZhuanhuanRequest, ZhuanhuanResult};

pub fn jisuan(catalog: &Catalog, req: &ZhuanhuanRequest) -> Result<ZhuanhuanResult> {
    let z = catalog
        .wuxingshi
        .iter()
        .find(|z| z.id == req.zhuanhuan_id)
        .ok_or_else(|| JxcgError::Data(format!("转换配方不存在：{}", req.zhuanhuan_id)))?;
    let cishu = req.cishu.max(1) as i64;
    let mut quejia: Vec<String> = Vec::new();

    let conga = match req.jiage.get(&z.cong) {
        Some(v) => *v,
        None => {
            quejia.push(z.cong.clone());
            0
        }
    };
    let daojia = match req.jiage.get(&z.dao) {
        Some(v) => *v,
        None => {
            quejia.push(z.dao.clone());
            0
        }
    };

    let chenggonglv_bp = z.chenggonglv_bp.clamp(1, 10_000);
    let p = chenggonglv_bp as f64 / 10_000.0;
    let qiwang_cishu = 1.0 / p;

    // 每次投入成本；期望成本 = 单次成本 × 期望次数 × 制造次数。
    let yici_chengben = conga
        .saturating_mul(z.xuyao as Tong)
        .saturating_add(z.jinbi)
        .saturating_add(z.jingli as Tong * req.jingli_danjia);
    let zong_chengben = (yici_chengben as f64 * qiwang_cishu * cishu as f64).round() as Tong;

    let qiwang_chuchan = p * cishu as f64;
    let shouru = (qiwang_chuchan * daojia as f64).round() as Tong;
    let feiyong = fee::jisuan_shouxufei(shouru, &req.shouxufei);
    let jingshouru = shouru.saturating_sub(feiyong);
    let lirun = jingshouru.saturating_sub(zong_chengben);
    let lirunlv_bp = if zong_chengben > 0 {
        (lirun as f64 / zong_chengben as f64 * 10_000.0).round() as i64
    } else {
        0
    };

    quejia.sort();
    quejia.dedup();

    Ok(ZhuanhuanResult {
        zhuanhuan_id: z.id.clone(),
        mingcheng: z.mingcheng.clone(),
        qiwang_cishu: qiwang_cishu * cishu as f64,
        zong_chengben,
        shouru,
        lirun,
        lirunlv_bp,
        quejia,
    })
}

/// 便捷函数：直接用 id 查价并计算。
pub fn jisuan_by_price(
    catalog: &Catalog,
    zhuanhuan_id: &str,
    jiage: &BTreeMap<String, Tong>,
    req: &ZhuanhuanRequest,
) -> Result<ZhuanhuanResult> {
    let mut next = req.clone();
    next.zhuanhuan_id = zhuanhuan_id.to_string();
    next.jiage = jiage.clone();
    jisuan(catalog, &next)
}
