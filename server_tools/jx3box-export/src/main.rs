// ============================================================================
// jx3box-export —— 构建期数据导出（P2）
//
// 从 JX3BOX 抓取 5 技艺配方目录/详情、物品、NPC 商店价、服务器列表，
// 导出为 data/*.toml（保持 jxcg_api 的 DTO / jxcg_core::store 的 schema）。
//
// 对上游礼貌（PLAN §7.1 接入原则）：
// - 并发默认 5（上限 6）、超时 15s、失败重试 1 次、UA `jxcg/0.1`；
// - 可重复运行（输出稳定排序；`--cache` 可复用原始 JSON，避免重复轰炸上游）。
//
// 赛季过滤（默认开启，见 season 模块）：只保留当前资料片（nDlc=max）+ 梓匠家具
// + 旧中间材料闭包，剔除已完成资料片的旧配方；`--all` 可导出全量做对比。
// ============================================================================

mod api;
mod error;
mod export;
mod http;
mod season;
mod util;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use jxcg_api::{
    Banben, Cailiao, Chuchan, Fuwuqi, Jiyi, Peifang, Pinlei, Wupin, WuxingshiFangan,
    WuxingshiHecheng, Zhuanhuan,
};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use error::{Error, Result};
use export::{DataBundle, ExportReport, PeifangOut};
use http::HttpClient;

const MAX_CONCURRENCY: usize = 6;

/// 赛季过滤目标（`--season auto|<n>` / `--all`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeasonTarget {
    /// `--all`：不按赛季过滤，导出全量配方（调试/对比）
    All,
    /// `--season auto`（默认）：当前资料片 = max(nDlc)
    Auto,
    /// `--season <n>`：指定资料片版本
    Fixed(i64),
}

/// `wuxingshi.toml` 读取结构（手工维护文件；缺文件/解析失败时用内置样例兜底）。
#[derive(Debug, Default, serde::Deserialize)]
struct WuxingshiFileCheck {
    #[serde(default)]
    zhuanhuan: Vec<Zhuanhuan>,
    #[serde(default)]
    fangan: Vec<WuxingshiFangan>,
    #[serde(default)]
    hecheng: Vec<WuxingshiHecheng>,
}

impl WuxingshiFileCheck {
    fn is_empty(&self) -> bool {
        self.zhuanhuan.is_empty() && self.fangan.is_empty() && self.hecheng.is_empty()
    }
}

/// 现有 `wupin.toml` 读取结构。
#[derive(Debug, Default, serde::Deserialize)]
struct WupinFileCheck {
    #[serde(default)]
    wupin: Vec<Wupin>,
}

/// 五行石相关物品 id：`wuxingshi.toml` 里所有引用到的物品
/// （`[[zhuanhuan]]` 的 cong/dao + `[[hecheng]]` 的 chanpin / cailiao / jieguo；
/// `[[fangan]]` 只声明方案、不直接引用物品）。
fn wuxingshi_item_ids(out: &Path) -> Vec<String> {
    let path = out.join("wuxingshi.toml");
    let file = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| toml::from_str::<WuxingshiFileCheck>(&text).ok())
        .filter(|file| !file.is_empty())
        .unwrap_or_else(|| WuxingshiFileCheck {
            zhuanhuan: export::default_wuxingshi(),
            ..WuxingshiFileCheck::default()
        });
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for conversion in &file.zhuanhuan {
        ids.insert(conversion.cong.clone());
        ids.insert(conversion.dao.clone());
    }
    for hecheng in &file.hecheng {
        ids.insert(hecheng.chanpin.clone());
        for cailiao in &hecheng.cailiao {
            ids.insert(cailiao.wupin_id.clone());
        }
        for jieguo in &hecheng.jieguo {
            ids.insert(jieguo.wupin_id.clone());
        }
    }
    ids.into_iter().collect()
}

/// 现有 `wupin.toml` 里手工维护的五行石物品（重导出时原样保留）。
///
/// 五行石 1~8 级物品是手工补进 wupin.toml 的（合成/精炼页需要），它们不在配方引用里，
/// 必须按「现有五行石条目」继承名称/分类，否则重导出会丢物品（离线缓存里也没有它们的旧数据）。
fn existing_wuxingshi_items(out: &Path) -> BTreeMap<String, Wupin> {
    std::fs::read_to_string(out.join("wupin.toml"))
        .ok()
        .and_then(|text| toml::from_str::<WupinFileCheck>(&text).ok())
        .unwrap_or_default()
        .wupin
        .into_iter()
        .filter(|item| item.pinlei == Pinlei::Wuxingshi)
        .map(|item| (item.id.clone(), item))
        .collect()
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("\n[错误] {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = parse_args()?;
    let started = Instant::now();
    let generated_unix = util::now_unix();
    println!("jx3box-export —— 从 JX3BOX 导出 data/*.toml");
    println!(
        "输出目录：{}；生成日期：{}（北京）；并发 {}；超时 {}s",
        args.out.display(),
        util::date_iso_bj(generated_unix),
        args.concurrency,
        args.timeout_secs
    );
    match args.season {
        SeasonTarget::All => println!("赛季过滤：关闭（--all 全量导出）"),
        SeasonTarget::Auto => println!("赛季过滤：auto（当前资料片 = max(nDlc)）"),
        SeasonTarget::Fixed(dlc) => println!("赛季过滤：指定 nDlc={dlc}"),
    }
    if args.offline {
        println!("离线模式：只读 --cache 缓存，不联网");
    }

    let client = HttpClient::new(
        args.timeout_secs,
        args.cache.clone(),
        args.offline,
        args.refresh_cache,
    )?;

    // ---- 1/6 配方目录（5 技艺） ----
    println!("\n[1/6] 拉取配方目录");
    let mut tasks: Vec<(&'static str, api::CatalogEntry)> = Vec::new();
    let mut catalog_by_tab: Vec<(String, usize)> = Vec::new();
    for (tab, jiyi) in api::JIYI_TABS {
        let entries = api::fetch_catalog(&client, tab).await?;
        println!(
            "  {tab:<10}（{}）：有效 {} 条",
            jiyi.mingcheng(),
            entries.len()
        );
        catalog_by_tab.push((tab.to_string(), entries.len()));
        let take = args.limit.unwrap_or(usize::MAX);
        tasks.extend(entries.into_iter().take(take).map(|entry| (tab, entry)));
    }
    let total = tasks.len();
    if total == 0 {
        return Err(Error::Data(
            "配方目录为空，检查上游接口是否变更".to_string(),
        ));
    }

    // ---- 2/6 配方详情（并发 + 进度） ----
    println!("\n[2/6] 拉取配方详情（共 {total} 条）");
    let (all_details, detail_failed) =
        fetch_details(&client, tasks, args.concurrency, started).await;

    // ---- 3/6 赛季过滤（nDlc） ----
    let current_dlc = match args.season {
        SeasonTarget::All => None,
        SeasonTarget::Auto => season::detect_current_dlc(&all_details),
        SeasonTarget::Fixed(dlc) => Some(dlc),
    };
    if args.season == SeasonTarget::Auto && current_dlc.is_none() {
        eprintln!("  [提示] 配方详情里没有 nDlc 字段，回退为全量导出");
    }
    let filter = season::filter_by_season(&all_details, current_dlc);
    let removed_examples: Vec<String> = all_details
        .iter()
        .zip(filter.kept.iter())
        .filter(|(_, kept)| !**kept)
        .map(|(detail, _)| detail.name.clone())
        .take(20)
        .collect();
    let closure_recipes: Vec<String> = filter
        .closure_added
        .iter()
        .filter_map(|index| all_details.get(*index))
        .map(|detail| detail.name.clone())
        .collect();
    let old_home_removed_count = filter.old_home_removed.len();
    let old_home_examples: Vec<String> = filter
        .old_home_removed
        .iter()
        .filter_map(|index| all_details.get(*index))
        .map(|detail| detail.name.clone())
        .take(10)
        .collect();
    let kept_by_tab = season::count_by_tab(&all_details, &filter.kept, true);
    let removed_by_tab = season::count_by_tab(&all_details, &filter.kept, false);
    let (kept_current_count, kept_furniture_count) = match current_dlc {
        Some(dlc) => (
            all_details
                .iter()
                .filter(|detail| detail.n_dlc == Some(dlc))
                .count(),
            all_details
                .iter()
                .zip(filter.kept.iter())
                .filter(|(detail, kept)| **kept && detail.tab == "furniture")
                .count(),
        ),
        None => (0, 0),
    };
    println!(
        "\n[3/6] 赛季过滤（nDlc={}）",
        current_dlc
            .map(|dlc| dlc.to_string())
            .unwrap_or_else(|| "—（未过滤）".to_string())
    );
    match current_dlc {
        Some(dlc) => {
            let season_name = season::season_mingcheng(dlc).unwrap_or("当前资料片");
            println!(
                "  保留 {} 条（本资料片 nDlc={dlc}（{season_name}）{kept_current_count} + 梓匠 {kept_furniture_count} + 旧中间材料闭包 {}），剔除 {} 条",
                filter.kept_count(),
                closure_recipes.len(),
                filter.removed_count()
            );
            println!("  保留（按技艺）：{}", tab_summary(&kept_by_tab));
            println!("  剔除（按技艺）：{}", tab_summary(&removed_by_tab));
            if old_home_removed_count > 0 {
                println!(
                    "  旧家园家具成品剔除 {old_home_removed_count} 条（梓匠产物 10_，依赖绑定/不可获得材料如徽墨 5_3843，无可靠价格来源）：{}",
                    old_home_examples.join(" / ")
                );
            }
            if !closure_recipes.is_empty() {
                println!(
                    "  闭包补齐 {} 条旧中间材料配方：{}",
                    closure_recipes.len(),
                    closure_recipes.join(" / ")
                );
            }
            if !removed_examples.is_empty() {
                println!("  剔除示例：{}", removed_examples.join(" / "));
            }
        }
        None => println!("  未过滤：保留全部 {} 条配方", filter.kept_count()),
    }

    let details: Vec<api::RecipeDetail> = all_details
        .into_iter()
        .zip(filter.kept.iter())
        .filter_map(|(detail, kept)| kept.then_some(detail))
        .collect();

    // ---- 4/6 物品（保留配方的材料 + 产物 + 五行石） ----
    let mut material_ids: BTreeSet<String> = BTreeSet::new();
    let mut product_ids: BTreeSet<String> = BTreeSet::new();
    for detail in &details {
        for (item_type, index, _) in &detail.cailiao {
            material_ids.insert(api::item_id(*item_type, *index));
        }
        for (item_type, index, _, _, _) in &detail.chuchan {
            product_ids.insert(api::item_id(*item_type, *index));
        }
    }
    let wuxingshi_ids = wuxingshi_item_ids(&args.out);
    // 手工维护的五行石条目（1~8 级）原样继承，避免重导出丢失/离线缓存缺数据。
    let manual_wupin = existing_wuxingshi_items(&args.out);
    let mut item_ids: BTreeSet<String> = BTreeSet::new();
    item_ids.extend(material_ids.iter().cloned());
    item_ids.extend(product_ids.iter().cloned());
    item_ids.extend(wuxingshi_ids.iter().cloned());
    item_ids.extend(manual_wupin.keys().cloned());
    // 已有手工条目的五行石不再请求上游（名称/分类沿用人工校验过的版本）。
    let fetch_ids: Vec<String> = item_ids
        .iter()
        .filter(|item_id| !manual_wupin.contains_key(*item_id))
        .cloned()
        .collect();
    println!(
        "\n[4/6] 拉取物品（共 {} 个：材料 {} / 产物 {} / 五行石 {}；其中 {} 个手工五行石沿用现有条目，请求 {} 个）",
        item_ids.len(),
        material_ids.len(),
        product_ids.len(),
        wuxingshi_ids.len().max(manual_wupin.len()),
        manual_wupin.len(),
        fetch_ids.len()
    );
    let (items, item_missing) = fetch_items(&client, &fetch_ids, args.concurrency, started).await;

    // 中间产物清单（保留配方内 产物 ∩ 材料）：core 的自制成本展开依赖它，日志便于核对。
    let intermediate_items: Vec<(String, String)> = filter
        .intermediate
        .iter()
        .map(|item_id| {
            let name = items
                .get(item_id)
                .map(|info| info.name.clone())
                .unwrap_or_else(|| "未知物品".to_string());
            (item_id.clone(), name)
        })
        .collect();
    if !intermediate_items.is_empty() {
        println!(
            "  中间产物（保留配方内，共 {} 个）：{}",
            intermediate_items.len(),
            intermediate_items
                .iter()
                .map(|(id, name)| format!("{name}({id})"))
                .collect::<Vec<_>>()
                .join(" / ")
        );
    }
    // 验收提示：饺子皮/饺子馅是旧赛季配方产物，过滤后不应再作为中间产物出现。
    for (id, name) in &intermediate_items {
        if id == "5_5269" || id == "5_5270" {
            eprintln!("  [警告] 中间产物里仍有旧赛季物品 {name}（{id}），请检查赛季过滤规则");
        }
    }

    // ---- 5/6 NPC 商店价 + 服务器 ----
    println!("\n[5/6] 拉取 NPC 商店价与服务器列表");
    let craft_prices = api::fetch_craft_prices(&client).await?;
    println!(
        "  商店价：{} 条（随后只保留保留配方引用物品的条目）",
        craft_prices.len()
    );
    let fuwuqi = api::fetch_servers(&client).await?;
    println!("  服务器：{} 个", fuwuqi.len());

    // ---- 6/6 组装 + 写盘 ----
    println!("\n[6/6] 生成 data/*.toml");
    let wupin = build_wupin(
        &item_ids,
        &items,
        &manual_wupin,
        &material_ids,
        &product_ids,
        &wuxingshi_ids,
        &craft_prices,
    );
    let peifang = build_peifang(details);

    // 详情失败太多说明上游异常（或本地网络差），宁可不覆盖数据。
    if args.limit.is_none() && detail_failed * 10 > total {
        return Err(Error::Data(format!(
            "配方详情失败 {detail_failed}/{total}（超过 10%），为避免写入残缺数据已中止"
        )));
    }

    let banben_note = match current_dlc {
        Some(dlc) => {
            let season_name = season::season_mingcheng(dlc).unwrap_or("当前资料片");
            format!("50级·{season_name}（nDlc={dlc}）· 已剔除旧赛季配方")
        }
        None => "50级·全量配方（未按赛季过滤）".to_string(),
    };
    let bundle = DataBundle {
        generated_unix,
        banben: Banben {
            level: 50,
            season: 3,
            modified: util::modified_date(generated_unix),
            note: banben_note,
        },
        wupin,
        peifang,
        fuwuqi,
    };

    let (network_requests, cache_hits, http_failures) = client.stats();
    let mut report = ExportReport {
        catalog_by_tab,
        detail_failed,
        item_missing,
        network_requests,
        cache_hits,
        http_failures,
        elapsed_ms: started.elapsed().as_millis(),
        current_dlc,
        kept_current_count,
        kept_furniture_count,
        kept_by_tab: to_named_counts(&kept_by_tab),
        removed_by_tab: to_named_counts(&removed_by_tab),
        closure_recipes,
        intermediate_items,
        removed_examples,
        old_home_removed_count,
        old_home_examples,
        ..ExportReport::default()
    };
    export::write_data_dir(&args.out, &bundle, &mut report)?;
    report.elapsed_ms = started.elapsed().as_millis();

    print_summary(&bundle, &report);
    Ok(())
}

/// 技艺 tab → 中文名（日志/README 用）。
fn tab_mingcheng(tab: &str) -> String {
    api::jiyi_of_tab(tab)
        .map(|jiyi| jiyi.mingcheng().to_string())
        .unwrap_or_else(|| tab.to_string())
}

/// `[(tab, 条数)]` → `[(中文技艺名, 条数)]`。
fn to_named_counts(counts: &[(&'static str, usize)]) -> Vec<(String, usize)> {
    counts
        .iter()
        .map(|(tab, count)| (tab_mingcheng(tab), *count))
        .collect()
}

/// 校验汇总用：「烹饪 30 / 制药 20 / …」。
fn tab_summary(counts: &[(&'static str, usize)]) -> String {
    to_named_counts(counts)
        .into_iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect::<Vec<_>>()
        .join(" / ")
}

// ============================================================================
// 并发抓取
// ============================================================================

async fn fetch_details(
    client: &HttpClient,
    tasks: Vec<(&'static str, api::CatalogEntry)>,
    concurrency: usize,
    started: Instant,
) -> (Vec<api::RecipeDetail>, usize) {
    let total = tasks.len();
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut set: JoinSet<(usize, Result<api::RecipeDetail>)> = JoinSet::new();
    for (index, (tab, entry)) in tasks.into_iter().enumerate() {
        let client = client.clone();
        let semaphore = semaphore.clone();
        set.spawn(async move {
            let permit = match semaphore.acquire_owned().await {
                Ok(permit) => permit,
                Err(err) => {
                    return (index, Err(Error::Http(format!("并发控制失败：{err}"))));
                }
            };
            let result = api::fetch_recipe_detail(&client, tab, &entry).await;
            drop(permit);
            (index, result)
        });
    }

    let mut results: Vec<(usize, api::RecipeDetail)> = Vec::with_capacity(total);
    let mut failed = 0usize;
    let mut done = 0usize;
    while let Some(joined) = set.join_next().await {
        done += 1;
        match joined {
            Ok((index, Ok(detail))) => results.push((index, detail)),
            Ok((_, Err(err))) => {
                failed += 1;
                if failed <= 10 {
                    eprintln!("  [失败] {err}");
                }
            }
            Err(err) => {
                failed += 1;
                if failed <= 10 {
                    eprintln!("  [失败] 任务异常：{err}");
                }
            }
        }
        if done.is_multiple_of(100) || done == total {
            println!(
                "  详情 {done}/{total}（失败 {failed}，已用 {:.1}s）",
                started.elapsed().as_secs_f64()
            );
        }
    }
    results.sort_by_key(|(index, _)| *index);
    let details = results.into_iter().map(|(_, detail)| detail).collect();
    (details, failed)
}

async fn fetch_items(
    client: &HttpClient,
    item_ids: &[String],
    concurrency: usize,
    started: Instant,
) -> (BTreeMap<String, api::ItemInfo>, usize) {
    let total = item_ids.len();
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut set: JoinSet<(usize, String, Result<Option<api::ItemInfo>>)> = JoinSet::new();
    for (index, item_id) in item_ids.iter().enumerate() {
        let client = client.clone();
        let semaphore = semaphore.clone();
        let item_id = item_id.clone();
        set.spawn(async move {
            let permit = match semaphore.acquire_owned().await {
                Ok(permit) => permit,
                Err(err) => {
                    return (
                        index,
                        item_id,
                        Err(Error::Http(format!("并发控制失败：{err}"))),
                    );
                }
            };
            let result = api::fetch_item(&client, &item_id).await;
            drop(permit);
            (index, item_id, result)
        });
    }

    let mut items: BTreeMap<String, api::ItemInfo> = BTreeMap::new();
    let mut missing = 0usize;
    let mut done = 0usize;
    let mut errors = 0usize;
    while let Some(joined) = set.join_next().await {
        done += 1;
        match joined {
            Ok((_, item_id, Ok(Some(info)))) => {
                items.insert(item_id, info);
            }
            Ok((_, item_id, Ok(None))) => {
                missing += 1;
                if missing <= 10 {
                    eprintln!("  [缺失] 物品 {item_id} 在 item_merged 中不存在");
                }
            }
            Ok((_, item_id, Err(err))) => {
                missing += 1;
                errors += 1;
                if errors <= 10 {
                    eprintln!("  [失败] 物品 {item_id}：{err}");
                }
            }
            Err(err) => {
                missing += 1;
                errors += 1;
                if errors <= 10 {
                    eprintln!("  [失败] 物品任务异常：{err}");
                }
            }
        }
        if done.is_multiple_of(100) || done == total {
            println!(
                "  物品 {done}/{total}（缺失 {missing}，已用 {:.1}s）",
                started.elapsed().as_secs_f64()
            );
        }
    }
    (items, missing)
}

// ============================================================================
// 组装 wupin / peifang
// ============================================================================

fn build_wupin(
    item_ids: &BTreeSet<String>,
    items: &BTreeMap<String, api::ItemInfo>,
    manual_wupin: &BTreeMap<String, Wupin>,
    material_ids: &BTreeSet<String>,
    product_ids: &BTreeSet<String>,
    wuxingshi_ids: &[String],
    craft_prices: &BTreeMap<u32, i64>,
) -> Vec<Wupin> {
    let mut wuxingshi: BTreeSet<&str> = wuxingshi_ids.iter().map(String::as_str).collect();
    wuxingshi.extend(manual_wupin.keys().map(String::as_str));
    let mut wupin = Vec::with_capacity(item_ids.len());
    for item_id in item_ids {
        // 手工维护的五行石条目：原样保留（名称/分类已人工校验，离线重导也不丢）。
        if let Some(manual) = manual_wupin.get(item_id) {
            wupin.push(manual.clone());
            continue;
        }
        let info = items.get(item_id);
        let (mingcheng, type_label) = match info {
            Some(info) => (info.name.clone(), info.type_label.clone()),
            None => (format!("未知物品（{item_id}）"), String::new()),
        };
        let is_material = material_ids.contains(item_id);
        let is_product = product_ids.contains(item_id);
        // 只有类型 5（普通物品）可能与 craft/price 的 ItemIndex 对应。
        let shangdianjia = item_id
            .strip_prefix("5_")
            .and_then(|index| index.parse::<u32>().ok())
            .and_then(|index| craft_prices.get(&index).copied());
        let (pinlei, rough) = classify_pinlei(
            &type_label,
            is_material,
            is_product,
            wuxingshi.contains(item_id.as_str()),
            shangdianjia.is_some(),
        );
        let laiyuan = match (info.is_some(), rough) {
            (false, _) => Some("jx3box(缺失)".to_string()),
            (true, true) => Some("jx3box(粗分)".to_string()),
            (true, false) => Some("jx3box".to_string()),
        };
        wupin.push(Wupin {
            id: item_id.clone(),
            mingcheng,
            pinlei,
            // 堆叠数上游未提供：宁缺毋滥，前端按 None 处理。
            dui: None,
            shangdianjia,
            laiyuan,
        });
    }
    wupin.sort_by_key(|item| util::item_sort_key(&item.id));
    wupin
}

/// 物品粗分（PLAN §7 只要求 cailiao/chengpin/xiaohao/wuxingshi 四类）。
///
/// 返回 `(类别, 是否粗分兜底)`；兜底项会在 `laiyuan` 里标注，便于后续人工校正。
fn classify_pinlei(
    type_label: &str,
    is_material: bool,
    is_product: bool,
    in_wuxingshi: bool,
    is_vendor_material: bool,
) -> (Pinlei, bool) {
    if in_wuxingshi || type_label == "五行石" {
        return (Pinlei::Wuxingshi, false);
    }
    if type_label == "材料" {
        return (Pinlei::Cailiao, false);
    }
    // craft/price 里的都是 NPC 出售的生活材料，分类是确定的。
    if is_vendor_material {
        return (Pinlei::Cailiao, false);
    }
    if is_product && !is_material {
        // 配方产物统一算成品（食物/装备/家具都是拿去卖的东西）
        return (Pinlei::Chengpin, false);
    }
    if is_material {
        return (Pinlei::Cailiao, true);
    }
    if matches!(
        type_label,
        "食物" | "药品" | "宴席" | "小吃" | "丹药" | "酒" | "消耗品"
    ) {
        return (Pinlei::Xiaohao, false);
    }
    (Pinlei::Cailiao, true)
}

fn build_peifang(details: Vec<api::RecipeDetail>) -> Vec<PeifangOut> {
    let mut peifang = Vec::with_capacity(details.len());
    for detail in details {
        let Some(jiyi) = api::jiyi_of_tab(&detail.tab) else {
            eprintln!("  [跳过] 未知技艺 tab：{}", detail.tab);
            continue;
        };
        let cailiao = detail
            .cailiao
            .iter()
            .map(|(item_type, index, count)| Cailiao {
                wupin_id: api::item_id(*item_type, *index),
                shuliang: *count,
            })
            .collect();
        let chuchan = detail
            .chuchan
            .iter()
            .map(|(item_type, index, min, max, quanzhong)| Chuchan {
                wupin_id: api::item_id(*item_type, *index),
                min: *min,
                max: *max,
                quanzhong: *quanzhong,
            })
            .collect();
        peifang.push(PeifangOut {
            id: format!("{}_{}", detail.tab, detail.game_id),
            mingcheng: detail.name,
            jiyi,
            dengji: detail.require_level,
            jingli: detail.cost_vigor,
            // 配方表里没有固定金币制作费，统一 0（后续如发现字段再补）
            zhizuofei: 0,
            laiyuan: Some("jx3box".to_string()),
            cailiao,
            chuchan,
        });
    }
    peifang.sort_by(|a, b| {
        jiyi_order(a.jiyi)
            .cmp(&jiyi_order(b.jiyi))
            .then_with(|| a.dengji.cmp(&b.dengji))
            .then_with(|| peifang_game_id(&a.id).cmp(&peifang_game_id(&b.id)))
    });
    peifang
}

fn jiyi_order(jiyi: Jiyi) -> u8 {
    match jiyi {
        Jiyi::Pengren => 0,
        Jiyi::Zhiyao => 1,
        Jiyi::Fengren => 2,
        Jiyi::Zhuzhao => 3,
        Jiyi::Zijiang => 4,
    }
}

/// `cooking_437` → 437（用于同技艺内按游戏 id 排序）。
fn peifang_game_id(id: &str) -> i64 {
    id.rsplit_once('_')
        .and_then(|(_, number)| number.parse().ok())
        .unwrap_or(i64::MAX)
}

// ============================================================================
// 参数 / 汇总
// ============================================================================

#[derive(Debug)]
struct Args {
    out: PathBuf,
    cache: Option<PathBuf>,
    offline: bool,
    refresh_cache: bool,
    concurrency: usize,
    limit: Option<usize>,
    timeout_secs: u64,
    /// 赛季过滤目标（默认 auto）
    season: SeasonTarget,
}

fn usage() -> String {
    [
        "用法：cargo run -p jx3box-export -- [选项]",
        "",
        "选项：",
        "  --out <目录>        输出目录（默认 data）",
        "  --cache <目录>      原始 JSON 缓存目录（默认关闭；调试时可复用，如 .cache/jx3box）",
        "  --offline           只读缓存不联网（需要 --cache）",
        "  --refresh-cache     忽略已有缓存，强制重新拉取",
        "  --concurrency <n>   并发数（默认 5，上限 6）",
        "  --timeout <秒>      单请求超时（默认 15）",
        "  --limit <n>         每技艺最多抓 n 条（调试用，正式导出不要加）",
        "  --season <auto|n>   按资料片过滤配方：auto = max(nDlc)（默认），n = 指定 nDlc",
        "  --all               不过滤，导出全量配方（调试/对比）",
        "  -h, --help          打印帮助",
    ]
    .join("\n")
}

fn parse_args() -> Result<Args> {
    let mut args = Args {
        out: PathBuf::from("data"),
        cache: None,
        offline: false,
        refresh_cache: false,
        concurrency: 5,
        limit: None,
        timeout_secs: 15,
        season: SeasonTarget::Auto,
    };
    let mut season_explicit = false;
    let mut all_explicit = false;
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            "--out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| Error::Data("--out 缺少参数".into()))?;
                args.out = PathBuf::from(value);
            }
            "--cache" => {
                let value = iter
                    .next()
                    .ok_or_else(|| Error::Data("--cache 缺少参数".into()))?;
                args.cache = Some(PathBuf::from(value));
            }
            "--offline" => args.offline = true,
            "--refresh-cache" => args.refresh_cache = true,
            "--concurrency" => {
                let value = iter
                    .next()
                    .ok_or_else(|| Error::Data("--concurrency 缺少参数".into()))?;
                let parsed: usize = value
                    .parse()
                    .map_err(|_| Error::Data(format!("--concurrency 不是数字：{value}")))?;
                args.concurrency = parsed.clamp(1, MAX_CONCURRENCY);
            }
            "--timeout" => {
                let value = iter
                    .next()
                    .ok_or_else(|| Error::Data("--timeout 缺少参数".into()))?;
                args.timeout_secs = value
                    .parse()
                    .map_err(|_| Error::Data(format!("--timeout 不是数字：{value}")))?;
            }
            "--limit" => {
                let value = iter
                    .next()
                    .ok_or_else(|| Error::Data("--limit 缺少参数".into()))?;
                let parsed: usize = value
                    .parse()
                    .map_err(|_| Error::Data(format!("--limit 不是数字：{value}")))?;
                args.limit = Some(parsed.max(1));
            }
            "--season" => {
                let value = iter
                    .next()
                    .ok_or_else(|| Error::Data("--season 缺少参数".into()))?;
                args.season = if value == "auto" {
                    SeasonTarget::Auto
                } else {
                    let dlc: i64 = value.parse().map_err(|_| {
                        Error::Data(format!("--season 只支持 auto 或正整数：{value}"))
                    })?;
                    if dlc <= 0 {
                        return Err(Error::Data(format!("--season 必须是正整数：{value}")));
                    }
                    SeasonTarget::Fixed(dlc)
                };
                season_explicit = true;
            }
            "--all" => {
                args.season = SeasonTarget::All;
                all_explicit = true;
            }
            other => {
                return Err(Error::Data(format!("未知参数：{other}\n\n{}", usage())));
            }
        }
    }
    if args.offline && args.cache.is_none() {
        return Err(Error::Data("--offline 需要同时指定 --cache".into()));
    }
    if season_explicit && all_explicit {
        return Err(Error::Data(
            "--all 与 --season 不能同时使用（--all 已表示不过滤）".into(),
        ));
    }
    Ok(args)
}

fn print_summary(bundle: &DataBundle, report: &ExportReport) {
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
    let catalog_summary = report
        .catalog_by_tab
        .iter()
        .map(|(tab, count)| format!("{tab} {count}"))
        .collect::<Vec<_>>()
        .join(" / ");
    println!("\n== 导出完成 ==");
    println!("目录有效条数：{catalog_summary}");
    if let Some(dlc) = report.current_dlc {
        let season_name = season::season_mingcheng(dlc).unwrap_or("当前资料片");
        let removed_total: usize = report.removed_by_tab.iter().map(|(_, count)| *count).sum();
        let removed_summary = report
            .removed_by_tab
            .iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect::<Vec<_>>()
            .join(" / ");
        println!(
            "赛季过滤：nDlc={dlc}（{season_name}）；保留 {} 条（本资料片 {} + 梓匠 {} + 闭包 {}），剔除 {} 条（{removed_summary}）",
            bundle.peifang.len(),
            report.kept_current_count,
            report.kept_furniture_count,
            report.closure_recipes.len(),
            removed_total,
        );
        if !report.removed_examples.is_empty() {
            println!("  剔除示例：{}", report.removed_examples.join(" / "));
        }
        if report.old_home_removed_count > 0 {
            println!(
                "  旧家园家具成品剔除 {} 条（梓匠产物 10_）：{}",
                report.old_home_removed_count,
                report.old_home_examples.join(" / ")
            );
        }
        if !report.closure_recipes.is_empty() {
            println!(
                "  闭包补齐的旧中间材料配方（{} 条）：{}",
                report.closure_recipes.len(),
                report.closure_recipes.join(" / ")
            );
        }
    } else {
        println!("赛季过滤：未启用（--all 全量导出）");
    }
    println!("配方 {} 条（{peifang_summary}）", bundle.peifang.len());
    println!(
        "物品 {} 条（{pinlei_summary}）；其中有 NPC 商店价 {} 条",
        bundle.wupin.len(),
        report.shangdian_count
    );
    if !report.intermediate_items.is_empty() {
        println!(
            "中间产物 {} 个（产物 ∩ 材料）：{}",
            report.intermediate_items.len(),
            report
                .intermediate_items
                .iter()
                .map(|(id, name)| format!("{name}({id})"))
                .collect::<Vec<_>>()
                .join(" / ")
        );
    }
    println!("服务器 {} 个", bundle.fuwuqi.len());
    println!(
        "网络：请求 {} 次 / 缓存命中 {} 次 / 最终失败 {} 次；配方详情失败 {}，物品缺失 {}",
        report.network_requests,
        report.cache_hits,
        report.http_failures,
        report.detail_failed,
        report.item_missing
    );
    println!("引用检查：{} 个物品引用缺失（应为 0）", report.refs_missing);
    println!("耗时 {:.1}s", report.elapsed_ms as f64 / 1000.0);
    for (name, bytes) in &report.files {
        println!("  data/{name:<16} {:>8.1} KB", *bytes as f64 / 1024.0);
    }
    if !report.warnings.is_empty() {
        println!(
            "\n告警 {} 条（详见 data/README.md）：",
            report.warnings.len()
        );
        for warning in report.warnings.iter().take(8) {
            println!("  - {warning}");
        }
    }
}

// 让 rustdoc 链接到 Peifang 契约（无实际调用，避免 unused import 告警）。
#[allow(dead_code)]
fn _api_contract_check(_: Peifang, _: Fuwuqi, _: Wupin) {}
