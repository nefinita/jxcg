//! 全配方收益排行。

use crate::craft;
use jxcg_api::{Catalog, JisuanRequest, PaihangRequest, PaihangRow, PaihangZhibiao};

/// 计算排行（按指标降序；缺价配方默认排除）。
pub fn paihang(catalog: &Catalog, req: &PaihangRequest) -> Vec<PaihangRow> {
    let mut rows = Vec::with_capacity(catalog.peifang.len());
    for peifang in &catalog.peifang {
        if let Some(jiyi) = req.jiyi
            && peifang.jiyi != jiyi
        {
            continue;
        }
        let jisuan_req = JisuanRequest {
            peifang_id: peifang.id.clone(),
            cishu: 1,
            guadan_cishu: 1,
            jingli_danjia: req.jingli_danjia,
            shouxufei: req.shouxufei.clone(),
            jiage: req.jiage.clone(),
            jiyi_dengji: req.jiyi_dengji,
        };
        let Ok(result) = craft::jisuan(catalog, &jisuan_req) else {
            continue;
        };
        if !result.quejia.is_empty() {
            continue;
        }
        rows.push(PaihangRow {
            peifang_id: peifang.id.clone(),
            mingcheng: peifang.mingcheng.clone(),
            jiyi: peifang.jiyi,
            lirun: result.lirun,
            lirunlv_bp: result.lirunlv_bp,
            mei_jingli: result.mei_jingli,
            jingli: peifang.jingli,
            quejia: false,
        });
    }

    rows.sort_by(|a, b| {
        let primary = match req.zhibiao {
            PaihangZhibiao::Lirun => b.lirun.cmp(&a.lirun),
            PaihangZhibiao::MeiJingli => b.mei_jingli.cmp(&a.mei_jingli),
            PaihangZhibiao::Lirunlv => b.lirunlv_bp.cmp(&a.lirunlv_bp),
        };
        primary.then_with(|| a.peifang_id.cmp(&b.peifang_id))
    });

    if let Some(limit) = req.limit {
        rows.truncate(limit);
    }
    rows
}
