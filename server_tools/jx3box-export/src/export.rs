//! 把抓到的数据落成 `data/*.toml`（保持 jxcg_core::store 已定的 schema）。
//!
//! 输出文件：
//! - `catalog.toml` / `wupin.toml` / `peifang.toml` / `fuwuqi.toml`：每次全量覆盖；
//! - `values.toml` / `wuxingshi.toml`：手工维护，已存在时保留（缺失才写模板）；
//! - `README.md`：生成信息/来源/口径/遗留问题。
//!
//! 写盘后会回读 TOML 校验（用 jxcg_api 的 DTO 反序列化 + 引用完整性检查），
//! 避免生成出 core 读不进去的坏数据。

use std::collections::BTreeSet;
use std::path::Path;

use jxcg_api::{
    Banben, Cailiao, Chuchan, Fuwuqi, Jiyi, Pinlei, Quzheng, Shouxufei, Tong, Wupin, Zhiji,
    Zhuanhuan,
};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::util::{date_iso_bj, modified_date};

/// 写入 data/ 的完整数据包。
pub struct DataBundle {
    pub generated_unix: i64,
    pub banben: Banben,
    pub wupin: Vec<Wupin>,
    pub peifang: Vec<PeifangOut>,
    pub fuwuqi: Vec<Fuwuqi>,
}

/// 导出统计（打印到终端 + 写进 data/README.md）。
#[derive(Debug, Default)]
pub struct ExportReport {
    pub peifang_by_jiyi: Vec<(String, usize)>,
    pub wupin_by_pinlei: Vec<(&'static str, usize)>,
    pub shangdian_count: usize,
    pub catalog_by_tab: Vec<(String, usize)>,
    /// 本次赛季过滤的目标资料片；None = `--all` 未过滤
    pub current_dlc: Option<i64>,
    /// 保留配方里 nDlc == current 的条数
    pub kept_current_count: usize,
    /// 保留配方里梓匠（furniture）的条数
    pub kept_furniture_count: usize,
    /// 保留配方按技艺（中文名）统计
    pub kept_by_tab: Vec<(String, usize)>,
    /// 剔除配方按技艺（中文名）统计
    pub removed_by_tab: Vec<(String, usize)>,
    /// 闭包补齐的旧中间材料配方名
    pub closure_recipes: Vec<String>,
    /// 保留配方派生的中间产物：(物品 id, 名称)
    pub intermediate_items: Vec<(String, String)>,
    /// 剔除配方示例名称
    pub removed_examples: Vec<String>,
    /// 因「旧家园家具成品（梓匠产物 10_）」剔除的配方数
    pub old_home_removed_count: usize,
    /// 旧家园家具成品剔除示例名称
    pub old_home_examples: Vec<String>,
    pub detail_failed: usize,
    pub item_missing: usize,
    pub network_requests: u64,
    pub cache_hits: u64,
    pub http_failures: u64,
    pub elapsed_ms: u128,
    pub files: Vec<(String, u64)>,
    pub refs_missing: usize,
    pub warnings: Vec<String>,
}

// ============================================================================
// TOML 文件结构（字段顺序刻意让标量在前、子表在后，TOML 才能正确嵌套）
// ============================================================================

#[derive(Serialize)]
struct CatalogFile {
    banben: Banben,
}

#[derive(Serialize)]
struct WupinFile {
    wupin: Vec<Wupin>,
}

/// 配方输出结构：字段顺序 = 标量 → `laiyuan` → 子表数组。
///
/// 注意：`jxcg_api::Peifang` 里 `laiyuan` 在 `cailiao/chuchan` 之后，
/// 直接序列化会把标量写进最后一个子表，所以这里单独定输出结构。
#[derive(Serialize, Debug, Clone)]
pub struct PeifangOut {
    pub id: String,
    pub mingcheng: String,
    pub jiyi: Jiyi,
    pub dengji: u32,
    pub jingli: u32,
    pub zhizuofei: Tong,
    pub laiyuan: Option<String>,
    pub cailiao: Vec<Cailiao>,
    pub chuchan: Vec<Chuchan>,
}

#[derive(Serialize)]
struct PeifangFile {
    peifang: Vec<PeifangOut>,
}

#[derive(Serialize)]
struct FuwuqiFile {
    fuwuqi: Vec<Fuwuqi>,
}

#[derive(Serialize)]
struct ValuesFile {
    zhiji: Vec<Zhiji>,
}

#[derive(Serialize)]
struct WuxingshiFile {
    zhuanhuan: Vec<Zhuanhuan>,
}

// ============================================================================
// 写盘
// ============================================================================

/// 写全量数据；返回统计里补齐 file 大小与引用检查结果。
pub fn write_data_dir(out: &Path, bundle: &DataBundle, report: &mut ExportReport) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let date = date_iso_bj(bundle.generated_unix);
    let modified = modified_date(bundle.generated_unix);

    // ---- catalog.toml ----
    let mut banben = bundle.banben.clone();
    banben.modified = modified;
    let catalog = CatalogFile {
        banben: banben.clone(),
    };
    let mut catalog_header =
        vec![
        "JXCG 随包数据版本（自动生成，勿手改；重新生成：cargo run -p jx3box-export -- --out data）"
            .to_string(),
        format!("生成日期：{date}（北京时间）；来源：JX3BOX（https://www.jx3box.com）"),
        format!(
            "数据量：配方 {} 条 / 物品 {} 条 / 服务器 {} 个",
            bundle.peifang.len(),
            bundle.wupin.len(),
            bundle.fuwuqi.len()
        ),
        format!("等级口径：{} 级（苍生铸世，2026-10-29 等级回退 50 级）", banben.level),
    ];
    catalog_header.extend(season_header_lines(report));
    let header = header_lines(&catalog_header);
    report
        .files
        .push(write_toml(&out.join("catalog.toml"), &header, &catalog)?);

    // ---- wupin.toml ----
    let mut pinlei_count: Vec<(&'static str, usize)> = vec![
        ("cailiao", 0),
        ("chengpin", 0),
        ("xiaohao", 0),
        ("wuxingshi", 0),
    ];
    for item in &bundle.wupin {
        let key = match item.pinlei {
            Pinlei::Cailiao => "cailiao",
            Pinlei::Chengpin => "chengpin",
            Pinlei::Xiaohao => "xiaohao",
            Pinlei::Wuxingshi => "wuxingshi",
        };
        if let Some(row) = pinlei_count.iter_mut().find(|(name, _)| *name == key) {
            row.1 += 1;
        }
    }
    report.wupin_by_pinlei = pinlei_count;
    report.shangdian_count = bundle
        .wupin
        .iter()
        .filter(|w| w.shangdianjia.is_some())
        .count();
    let wupin_header = header_lines(&[
        "物品（自动生成；来源：JX3BOX item_merged + craft/price）".to_string(),
        format!("生成日期：{date}（北京时间）"),
        "pinlei 为粗分（cailiao/chengpin/xiaohao/wuxingshi）；不确定的按材料处理并标 laiyuan=jx3box(粗分)".to_string(),
        "shangdianjia 只取 NPC 售卖价（craft/price）；物品表 Price 是卖给商店的价，不能当成本兜底，未采用".to_string(),
        "dui（一组堆叠数）上游接口未提供，暂时缺省；需要按组购买时由前端/设置兜底".to_string(),
    ]);
    report.files.push(write_toml(
        &out.join("wupin.toml"),
        &wupin_header,
        &WupinFile {
            wupin: bundle.wupin.clone(),
        },
    )?);

    // ---- peifang.toml ----
    let mut jiyi_count: Vec<(String, usize)> = Vec::new();
    for (jiyi, name) in Jiyi::ALL
        .into_iter()
        .zip(["烹饪", "制药", "缝纫", "铸造", "梓匠"])
    {
        let count = bundle.peifang.iter().filter(|p| p.jiyi == jiyi).count();
        jiyi_count.push((name.to_string(), count));
    }
    report.peifang_by_jiyi = jiyi_count;
    let summary = report
        .peifang_by_jiyi
        .iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect::<Vec<_>>()
        .join(" / ");
    let mut peifang_header_lines = vec![
        "生活技艺配方（自动生成，勿手改；来源：JX3BOX manufacture 目录 + 详情）".to_string(),
        format!("生成日期：{date}（北京时间）"),
        format!("覆盖 5 技艺 {} 条：{summary}", bundle.peifang.len()),
        "chuchan.quanzhong 是概率权重，分母 2^20 = 1048576（普通 1006633 / 精制 41943）；材料槽只留数量>0，产物槽只留权重>0".to_string(),
    ];
    peifang_header_lines.extend(season_header_lines(report));
    let peifang_header = header_lines(&peifang_header_lines);
    report.files.push(write_toml(
        &out.join("peifang.toml"),
        &peifang_header,
        &PeifangFile {
            peifang: bundle.peifang.clone(),
        },
    )?);

    // ---- fuwuqi.toml ----
    let fuwuqi_header = header_lines(&[
        "服务器（自动生成；来源：JX3BOX/jx3box-data data/server/*.json）".to_string(),
        format!("生成日期：{date}（北京时间）"),
        "id 是本地稳定键（行情缓存文件名/设置项）；JX3BOX 行情接口的 server 参数要用 mingcheng（中文名）".to_string(),
    ]);
    report.files.push(write_toml(
        &out.join("fuwuqi.toml"),
        &fuwuqi_header,
        &FuwuqiFile {
            fuwuqi: bundle.fuwuqi.clone(),
        },
    )?);

    // ---- values.toml / wuxingshi.toml：手工维护，已存在则保留 ----
    let values_path = out.join("values.toml");
    if values_path.is_file() {
        report
            .warnings
            .push("保留现有 values.toml（手工维护的值集，导出不覆盖）".to_string());
    } else {
        let values_header = header_lines(&[
            "值集：交易行费率 / 取整 / 保管费（自动生成的默认值；可手工调整）".to_string(),
            format!(
                "生成日期：{date}（北京时间）；等级口径：50 级·苍生铸世（2026-10-29 等级回退）"
            ),
            "id 沿用历史 live-130（用户设置兼容，勿改）；界面显示名已更新为「正式服·50级」"
                .to_string(),
            "口径来源：wind-jx3 / hidewnd 社区实现交叉验证，待游戏内实测校准（PLAN.md §6.2）"
                .to_string(),
        ]);
        report.files.push(write_toml(
            &values_path,
            &values_header,
            &ValuesFile {
                zhiji: default_zhiji(&date),
            },
        )?);
    }

    let wuxingshi_path = out.join("wuxingshi.toml");
    if wuxingshi_path.is_file() {
        report
            .warnings
            .push("保留现有 wuxingshi.toml（手工维护的五行石规则，待实机校准）".to_string());
    } else {
        let wuxingshi_header = header_lines(&[
            "五行石精炼（自动生成的样例骨架；手工维护）".to_string(),
            "规则来自社区整理、2026 版本待实机校准；界面可编辑".to_string(),
            "注意：同一等级存在可交易/绑定两个 id，后续校准按可交易版本归一".to_string(),
        ]);
        report.files.push(write_toml(
            &wuxingshi_path,
            &wuxingshi_header,
            &WuxingshiFile {
                zhuanhuan: default_wuxingshi(),
            },
        )?);
    }

    // ---- 回读校验 ----
    let check = verify(out)?;
    report.refs_missing = check.refs_missing;
    report.warnings.extend(check.warnings);

    // ---- data/README.md ----
    report.files.push(write_readme(out, bundle, report, &date)?);
    Ok(())
}

/// 赛季过滤的保留/剔除总数与摘要（README/日志复用）。
fn season_kept_total(report: &ExportReport) -> usize {
    report.kept_current_count + report.kept_furniture_count + report.closure_recipes.len()
}

fn season_removed_total(report: &ExportReport) -> usize {
    report.removed_by_tab.iter().map(|(_, count)| *count).sum()
}

/// 写进 catalog/peifang 头注释的赛季说明行。
fn season_header_lines(report: &ExportReport) -> Vec<String> {
    match report.current_dlc {
        Some(dlc) => {
            let season_name = crate::season::season_mingcheng(dlc).unwrap_or("当前资料片");
            vec![
                format!(
                    "赛季过滤：nDlc={dlc}（{season_name}）；保留 {} 条（本资料片 {} + 梓匠 5_ 产物 {} + 旧中间材料闭包 {}），剔除 {} 条（旧赛季 + 旧家园家具成品）",
                    season_kept_total(report),
                    report.kept_current_count,
                    report.kept_furniture_count,
                    report.closure_recipes.len(),
                    season_removed_total(report)
                ),
                format!(
                    "旧家园家具成品（梓匠产物 10_）剔除 {} 条：依赖绑定/不可获得阅读材料（徽墨等），NPC 无售价、无合成配方、交易行无稳定挂单",
                    report.old_home_removed_count
                ),
                "如需全量导出（含旧赛季/旧家园家具配方，仅调试/对比）：cargo run -p jx3box-export -- --out data --all"
                    .to_string(),
            ]
        }
        None => vec!["赛季过滤：未启用（--all 全量导出，含旧赛季配方）".to_string()],
    }
}

fn header_lines(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn write_toml<T: Serialize>(path: &Path, header: &str, value: &T) -> Result<(String, u64)> {
    let body = toml::to_string_pretty(value)?;
    let mut text = String::with_capacity(header.len() + body.len() + 1);
    text.push_str(header);
    text.push('\n');
    text.push_str(&body);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    std::fs::write(path, text.as_bytes())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("?")
        .to_string();
    Ok((name, text.len() as u64))
}

// ============================================================================
// 默认值（values / wuxingshi 模板）
// ============================================================================

/// 默认值集：5% HALF_UP + 保管费 80 银/24h（PLAN.md §6.2）。
pub fn default_zhiji(date: &str) -> Vec<Zhiji> {
    vec![Zhiji {
        // id 保持历史值 "live-130"（等级口径已改 50 级）：用户设置里已存该 id，改名会破坏旧设置。
        id: "live-130".to_string(),
        mingcheng: "正式服·50级".to_string(),
        note: format!(
            "50 级·苍生铸世：5% 四舍五入 + 保管费 80 银/24h（社区口径，实机校准后更新；生成于 {date}）"
        ),
        shouxufei: Shouxufei {
            shuilv_bp: 500,
            quzheng: Quzheng::HalfUp,
            baoguanfei: 8_000,
        },
    }]
}

/// 五行石转换样例（与仓库现有 data/wuxingshi.toml 一致，待实机校准）。
pub fn default_wuxingshi() -> Vec<Zhuanhuan> {
    vec![
        Zhuanhuan {
            id: "wuxing_1to3".to_string(),
            mingcheng: "一级→三级（13 合 1）".to_string(),
            cong: "5_24423".to_string(),
            dao: "5_24425".to_string(),
            xuyao: 13,
            jingli: 10,
            jinbi: 3_900,
            chenggonglv_bp: 10_000,
            laiyuan: Some("shequ-2023".to_string()),
        },
        Zhuanhuan {
            id: "wuxing_3to4".to_string(),
            mingcheng: "三级+一级→四级（成功率 48.78%）".to_string(),
            cong: "5_24425".to_string(),
            dao: "5_24426".to_string(),
            xuyao: 1,
            jingli: 10,
            jinbi: 8_900,
            chenggonglv_bp: 4_878,
            laiyuan: Some("shequ-2023".to_string()),
        },
    ]
}

// ============================================================================
// 回读校验
// ============================================================================

struct CheckResult {
    refs_missing: usize,
    warnings: Vec<String>,
}

#[derive(Deserialize)]
struct WupinCheck {
    wupin: Vec<Wupin>,
}

#[derive(Deserialize)]
struct PeifangCheck {
    peifang: Vec<jxcg_api::Peifang>,
}

fn read_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|err| Error::Io(format!("读取 {} 失败：{err}", path.display())))
}

fn parse_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = read_file(path)?;
    toml::from_str(&text)
        .map_err(|err| Error::Parse(format!("回读 {} 失败：{err}", path.display())))
}

fn verify(out: &Path) -> Result<CheckResult> {
    let wupin: WupinCheck = parse_toml(&out.join("wupin.toml"))?;
    let peifang: PeifangCheck = parse_toml(&out.join("peifang.toml"))?;
    if wupin.wupin.is_empty() {
        return Err(Error::Data("wupin.toml 为空".to_string()));
    }
    if peifang.peifang.is_empty() {
        return Err(Error::Data("peifang.toml 为空".to_string()));
    }

    let ids: BTreeSet<&str> = wupin.wupin.iter().map(|item| item.id.as_str()).collect();
    if ids.len() != wupin.wupin.len() {
        return Err(Error::Data("wupin.toml 存在重复物品 id".to_string()));
    }
    let peifang_ids: BTreeSet<&str> = peifang.peifang.iter().map(|p| p.id.as_str()).collect();
    if peifang_ids.len() != peifang.peifang.len() {
        return Err(Error::Data("peifang.toml 存在重复配方 id".to_string()));
    }

    let mut refs_missing = 0usize;
    let mut warnings = Vec::new();
    for recipe in &peifang.peifang {
        if recipe.chuchan.is_empty() {
            warnings.push(format!("配方 {} 没有产物槽", recipe.id));
        }
        if recipe.cailiao.is_empty() {
            warnings.push(format!("配方 {} 没有材料槽", recipe.id));
        }
        for cailiao in &recipe.cailiao {
            if !ids.contains(cailiao.wupin_id.as_str()) {
                refs_missing += 1;
                if warnings.len() < 20 {
                    warnings.push(format!(
                        "配方 {} 材料 {} 不在 wupin.toml",
                        recipe.id, cailiao.wupin_id
                    ));
                }
            }
        }
        for chuchan in &recipe.chuchan {
            if !ids.contains(chuchan.wupin_id.as_str()) {
                refs_missing += 1;
                if warnings.len() < 20 {
                    warnings.push(format!(
                        "配方 {} 产物 {} 不在 wupin.toml",
                        recipe.id, chuchan.wupin_id
                    ));
                }
            }
            if chuchan.quanzhong == 0 {
                return Err(Error::Data(format!("配方 {} 产物权重为 0", recipe.id)));
            }
        }
    }
    Ok(CheckResult {
        refs_missing,
        warnings,
    })
}

// ============================================================================
// data/README.md
// ============================================================================

fn write_readme(
    out: &Path,
    bundle: &DataBundle,
    report: &ExportReport,
    date: &str,
) -> Result<(String, u64)> {
    let peifang_summary = report
        .peifang_by_jiyi
        .iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect::<Vec<_>>()
        .join(" / ");
    let pinlei_summary = report
        .wupin_by_pinlei
        .iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect::<Vec<_>>()
        .join(" / ");
    let mut lines = Vec::new();
    lines.push("# data/ —— JXCG 随包数据".to_string());
    lines.push(String::new());
    lines.push(format!(
        "> 本目录由 `cargo run -p jx3box-export -- --out data` 生成（生成日期 {date}，北京时间）。"
    ));
    lines.push(String::new());
    lines.push("## 来源与授权".to_string());
    lines.push(String::new());
    lines.push("- 数据来自 [JX3BOX](https://www.jx3box.com) 社区接口（非官方、无 SLA）与游戏数据；仅本地计算使用，不再分发。".to_string());
    lines.push("- 配方：`node.jx3box.com/manufactures` + `manufacture/{type}/{id}`；物品：`resource/std/item_merged.{id}`；商店价：`craft/price`；服务器：`JX3BOX/jx3box-data` 仓库 `data/server/*.json`。".to_string());
    lines.push(
        "- 若上游反对再分发，改为「运行时拉取 / 用户导入」模式（见 PLAN.md §10）。".to_string(),
    );
    lines.push(String::new());
    lines.push("## 本次生成统计".to_string());
    lines.push(String::new());
    lines.push(format!(
        "- 数据版本：**{} 级**·苍生铸世（2026-10-29 资料片上线，角色/生活技艺等级回退到 50 级）",
        bundle.banben.level
    ));
    if let Some(dlc) = report.current_dlc {
        lines.push(format!(
            "- 赛季过滤：**nDlc={dlc}**；保留 {} 条 / 剔除 {} 条旧赛季配方（下详）",
            season_kept_total(report),
            season_removed_total(report)
        ));
    } else {
        lines.push("- 赛季过滤：**未启用**（`--all` 全量导出，含旧赛季配方）".to_string());
    }
    lines.push(format!(
        "- 配方：**{}** 条（{peifang_summary}）",
        bundle.peifang.len()
    ));
    if let Some(_dlc) = report.current_dlc {
        lines.push(format!(
            "- 剔除构成：旧赛季配方 {} 条 + 旧家园家具成品（`10_` 产物）{} 条",
            season_removed_total(report).saturating_sub(report.old_home_removed_count),
            report.old_home_removed_count
        ));
    }
    lines.push(format!(
        "- 物品：**{}** 条（{pinlei_summary}）；其中有 NPC 商店价 {} 条",
        bundle.wupin.len(),
        report.shangdian_count
    ));
    if !report.intermediate_items.is_empty() {
        lines.push(format!(
            "- 中间产物：**{}** 个（产物 ∩ 材料，core 按自制成本展开）",
            report.intermediate_items.len()
        ));
    }
    lines.push(format!("- 服务器：{} 个", bundle.fuwuqi.len()));
    if report.network_requests == 0 && report.cache_hits > 0 {
        lines.push(format!(
            "- 网络：本次复用原始 JSON 缓存（`--cache`，命中 {} 条 URL），未联网；要同步最新数据请去掉 `--cache` 重跑。",
            report.cache_hits
        ));
    } else {
        lines.push(format!(
            "- 网络：请求 {} 次 / 缓存命中 {} 次 / 最终失败 {} 次；配方详情失败 {} 条，物品缺失 {} 条；耗时 {:.1}s",
            report.network_requests,
            report.cache_hits,
            report.http_failures,
            report.detail_failed,
            report.item_missing,
            report.elapsed_ms as f64 / 1000.0
        ));
    }
    lines.push(String::new());

    // ---- 赛季过滤规则/明细 ----
    lines.push("## 赛季过滤（nDlc）".to_string());
    lines.push(String::new());
    match report.current_dlc {
        Some(dlc) => {
            let season_name = crate::season::season_mingcheng(dlc).unwrap_or("当前资料片");
            lines.push(format!(
                "当前正式服资料片 **nDlc={dlc}（{season_name}）**；导出器只保留「本赛季用得上的配方」，剔除已完成资料片的旧配方。"
            ));
            lines.push(String::new());
            lines.push("判定口径（用户确认）：材料价格只认 **NPC 售卖价 / 合成配方 / 交易行挂单** 三者之一；三者皆无 = 不可获得材料，会把配方长期拖成缺价。`10_` 旧家园家具成品正是这种情况（徽墨 `5_3843`/水纹纸/浣花笺/五彩墨等为绑定阅读材料），故整体剔除。".to_string());
            lines.push(String::new());
            lines.push("规则（`server_tools/jx3box-export/src/season.rs`）：".to_string());
            lines.push(String::new());
            lines.push(format!(
                "1. `nDlc == {dlc}` 的当前资料片配方：**{}** 条；",
                report.kept_current_count
            ));
            lines.push(format!(
                "2. 梓匠（`furniture`）中产物为 `5_` 的可交易材料/套件配方保留：**{}** 条（木板/卯榫/雕花/篾条/鱼胶、红荣素·臻、家园套件等，晋秩要用）；",
                report.kept_furniture_count
            ));
            lines.push(format!(
                "3. 旧中间材料闭包补齐：**{}** 条（产物被保留配方当材料的旧配方，迭代到不动点）。",
                report.closure_recipes.len()
            ));
            if !report.closure_recipes.is_empty() {
                lines.push(format!(
                    "   - 闭包配方：{}",
                    report.closure_recipes.join(" / ")
                ));
            }
            if !report.intermediate_items.is_empty() {
                lines.push(format!(
                    "   - 派生中间产物（{} 个）：{}",
                    report.intermediate_items.len(),
                    report
                        .intermediate_items
                        .iter()
                        .map(|(id, name)| format!("{name}({id})"))
                        .collect::<Vec<_>>()
                        .join(" / ")
                ));
            }
            if !report.kept_by_tab.is_empty() {
                lines.push(format!(
                    "   - 保留（按技艺）：{}",
                    report
                        .kept_by_tab
                        .iter()
                        .map(|(name, count)| format!("{name} {count}"))
                        .collect::<Vec<_>>()
                        .join(" / ")
                ));
            }
            if !report.removed_by_tab.is_empty() {
                lines.push(format!(
                    "   - 剔除（按技艺）：{}",
                    report
                        .removed_by_tab
                        .iter()
                        .map(|(name, count)| format!("{name} {count}"))
                        .collect::<Vec<_>>()
                        .join(" / ")
                ));
            }
            lines.push(format!(
                "4. 剔除梓匠中产物为 `10_` 的**旧家园家具成品**：**{}** 条（章草/偶作等；依赖绑定/不可获得阅读材料，无 NPC 价、无合成配方、交易行无稳定挂单，无可靠价格来源，本赛季不参与交易行计算）。",
                report.old_home_removed_count
            ));
            if !report.old_home_examples.is_empty() {
                lines.push(format!(
                    "   - 旧家园家具剔除示例：{}",
                    report.old_home_examples.join(" / ")
                ));
            }
            lines.push(String::new());
            if !report.removed_examples.is_empty() {
                lines.push(format!(
                    "已剔除（{} 条：旧赛季 + 旧家园家具）示例：{}。",
                    season_removed_total(report),
                    report.removed_examples.join(" / ")
                ));
                lines.push(String::new());
            }
            lines.push("- 切回全量（含旧赛季，仅调试/对比）：`cargo run -p jx3box-export -- --out data --all`".to_string());
            lines.push(
                "- 指定资料片：`--season 9`（数字是 nDlc）；默认 `--season auto` = max(nDlc)。"
                    .to_string(),
            );
        }
        None => {
            lines.push("未启用（`--all` 全量导出）：包含旧赛季配方，仅供调试/对比。".to_string());
            lines.push(String::new());
            lines
                .push("- 恢复默认赛季过滤：`cargo run -p jx3box-export -- --out data`".to_string());
        }
    }
    lines.push(String::new());

    lines.push("## 等级口径（50 级回退）".to_string());
    lines.push(String::new());
    lines.push("- **2026-10-29「苍生铸世」资料片上线后，角色等级回退到 50 级，生活技艺等级上限也是 50 级**；本数据 `[banben].level = 50`。".to_string());
    lines.push("- 配方 `dengji` 仍是游戏内原始 `RequireLevel`（对应 50 级上限口径），未做 130→50 的数值偏移。".to_string());
    lines.push("- 值集 id 沿用历史 `live-130`（用户设置里已存该 id，改名会破坏旧设置），显示名已改为「正式服·50级」。".to_string());
    lines.push(String::new());

    lines.push("## 五行石合成数据（手工维护）".to_string());
    lines.push(String::new());
    lines.push("- `wuxingshi.toml` **不由导出器覆盖**（只在新目录初始化时写模板），手工维护来源：游戏内精炼面板（2026-11）+ [JX3BOX 2026-11-08 攻略《面向萌新的五行石精炼方法与成本》](https://www.jx3box.com/community/6363) 与社区配方整理。".to_string());
    lines.push("- 结构：`[[fangan]]` 合成方案 + `[[hecheng]]` 单步配方（主产物 `chanpin` / 材料 `cailiao` / 精力 / 加工费 + 概率结果 `jieguo.bp` 万分比，未列出的余量为失败）+ 兼容旧 `[[zhuanhuan]]`。".to_string());
    lines.push("- **待校准**：合 6 意外出八级（连升两级）的概率面板未公示，`mianban_6` 预留 `bp=0` 结果槽，前端可用 `baoji_bp` 覆盖；三级/四级等概率为面板口径，实机校准后更新。".to_string());
    lines.push("- 五行石 1~8 级物品（`5_24423`~`5_24430`）由手工补进 `wupin.toml`，导出器按「现有五行石条目」**原样继承**，重导出不会丢失。".to_string());
    lines.push(String::new());

    lines.push("## 文件".to_string());
    lines.push(String::new());
    lines.push("| 文件 | 内容 | 生成方式 |".to_string());
    lines.push("|------|------|----------|".to_string());
    lines.push(
        "| `catalog.toml` | 数据版本 `[banben]` | 导出（版本号手工维护/计划校准） |".to_string(),
    );
    lines.push("| `wupin.toml` | 物品（名称/分类/商店价） | 导出 |".to_string());
    lines.push("| `peifang.toml` | 生活技艺配方（材料 8 槽 + 产物 8 槽权重） | 导出 |".to_string());
    lines.push("| `fuwuqi.toml` | 服务器（id/mingcheng/daju） | 导出 |".to_string());
    lines.push("| `wuxingshi.toml` | 五行石转换/合成方案/单步配方（`[[zhuanhuan]]`/`[[fangan]]`/`[[hecheng]]`） | **手工维护**（待实机校准） |".to_string());
    lines.push("| `values.toml` | 值集（费率/取整/保管费） | **手工维护**（默认 5% HALF_UP + 8000 铜/24h） |".to_string());
    lines.push(String::new());
    lines.push("## 口径与已知问题".to_string());
    lines.push(String::new());
    lines.push("- 产物权重分母固定 `2^20 = 1048576`（普通 1006633 / 精制 41943）；材料槽只保留数量>0，产物槽只保留权重>0。".to_string());
    lines.push("- 配方 id 形如 `cooking_437`（`{JX3BOX tab}_{游戏 ID}`）；`jiyi` 映射：cooking→pengren、medicine→zhiyao、tailoring→fengren、founding→zhuzhao、furniture→zijiang。".to_string());
    lines.push("- 物品 `pinlei` 为**粗分**（cailiao/chengpin/xiaohao/wuxingshi）：配方产物 → chengpin；五行石 → wuxingshi；其余按上游 `TypeLabel`，拿不准按 cailiao 并标 `laiyuan=jx3box(粗分)`。".to_string());
    lines.push("- `shangdianjia` 只取 NPC 售卖价（`craft/price`）；物品表 `Price` 是卖给商店的价格，不能当成本兜底，未采用。".to_string());
    lines.push(
        "- `dui`（一组堆叠数）上游未提供，暂缺省；如需按组购买请在设置里补默认值。".to_string(),
    );
    lines.push(
        "- 五行石精炼/合成规则不在 `manufacture` 接口里，`wuxingshi.toml` 手工维护（来源与待校准项见上文「五行石合成数据」一节）。"
            .to_string(),
    );
    lines.push("- 旧家园家具成品（`10_` 产物）配方已剔除：依赖绑定/不可获得的阅读材料（徽墨/水纹纸/浣花笺/五彩墨等），按「NPC 售卖 / 合成配方 / 交易行价格」三口径均无可靠价格，计入会长期缺价。".to_string());
    lines.push("- 服务器 `id` 为手工维护的 pinyin 映射（`util::pinyin_id`）；**JX3BOX 行情接口的 `server` 参数必须用中文名 `mingcheng`**。".to_string());
    lines.push(String::new());
    if !report.warnings.is_empty() {
        lines.push("### 生成提示/告警".to_string());
        lines.push(String::new());
        for warning in report.warnings.iter().take(20) {
            lines.push(format!("- {warning}"));
        }
        lines.push(String::new());
    }
    lines.push("## 重新生成".to_string());
    lines.push(String::new());
    lines.push("```sh".to_string());
    lines.push("# 需要联网；对上游礼貌（并发 5、超时 15s、失败重试 1 次）".to_string());
    lines.push("# 默认 --season auto = 当前资料片 max(nDlc)，剔除旧赛季配方".to_string());
    lines.push("cargo run -p jx3box-export -- --out data".to_string());
    lines.push("# 全量导出（含旧赛季配方，仅调试/对比）".to_string());
    lines.push("cargo run -p jx3box-export -- --out data --all".to_string());
    lines.push("# 指定资料片 nDlc（例如 9 = 苍生铸世）".to_string());
    lines.push("cargo run -p jx3box-export -- --out data --season 9".to_string());
    lines.push("# 调试/续跑：复用 .cache/jx3box 原始 JSON，只抓每类前 N 条".to_string());
    lines.push(
        "cargo run -p jx3box-export -- --out data --limit 5 --cache .cache/jx3box".to_string(),
    );
    lines.push("```".to_string());
    lines.push(String::new());

    let mut text = lines.join("\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let path = out.join("README.md");
    std::fs::write(&path, text.as_bytes())?;
    Ok(("README.md".to_string(), text.len() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peifang_toml_field_order_is_valid() {
        // 关键回归：laiyuan 必须在 cailiao/chuchan 之前，否则会被写进最后一个子表。
        let peifang = vec![PeifangOut {
            id: "cooking_1".to_string(),
            mingcheng: "测试".to_string(),
            jiyi: Jiyi::Pengren,
            dengji: 10,
            jingli: 20,
            zhizuofei: 0,
            laiyuan: Some("jx3box".to_string()),
            cailiao: vec![Cailiao {
                wupin_id: "5_1".to_string(),
                shuliang: 2,
            }],
            chuchan: vec![Chuchan {
                wupin_id: "5_2".to_string(),
                min: 1,
                max: 1,
                quanzhong: 1_048_576,
            }],
        }];
        let text = toml::to_string_pretty(&PeifangFile { peifang }).expect("序列化失败");
        assert!(text.contains("[[peifang]]"));
        assert!(text.contains("[[peifang.cailiao]]"));
        assert!(text.contains("[[peifang.chuchan]]"));
        // 回读仍能还原成 jxcg_api::Peifang（core 的数据契约）
        let parsed: PeifangCheck = toml::from_str(&text).expect("回读失败");
        assert_eq!(parsed.peifang.len(), 1);
        let recipe = &parsed.peifang[0];
        assert_eq!(recipe.laiyuan.as_deref(), Some("jx3box"));
        assert_eq!(recipe.cailiao.len(), 1);
        assert_eq!(recipe.chuchan.len(), 1);
    }

    #[test]
    fn default_values_match_plan() {
        let zhiji = default_zhiji("2026-11-12");
        assert_eq!(zhiji.len(), 1);
        assert_eq!(zhiji[0].shouxufei.shuilv_bp, 500);
        assert_eq!(zhiji[0].shouxufei.quzheng, Quzheng::HalfUp);
        assert_eq!(zhiji[0].shouxufei.baoguanfei, 8_000);
        let wuxing = default_wuxingshi();
        assert_eq!(wuxing.len(), 2);
        assert_eq!(wuxing[1].chenggonglv_bp, 4_878);
    }

    #[test]
    fn header_lines_format() {
        let header = header_lines(&["a".to_string(), "b".to_string()]);
        assert_eq!(header, "# a\n# b\n");
    }
}
