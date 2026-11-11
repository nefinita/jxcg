// ============================================================================
// jx3box-export —— 构建期数据导出（P2）
//
// 从 JX3BOX 抓取 5 技艺配方目录/详情、物品、NPC 商店价、服务器列表，
// 导出为 data/*.toml（保持 jxcg_api 的 DTO / jxcg_core::store 的 schema）。
//
// 对上游礼貌（PLAN §7.1 接入原则）：
// - 并发默认 5（上限 6）、超时 15s、失败重试 1 次、UA `jxcg/0.1`；
// - 可重复运行（输出稳定排序；`--cache` 可复用原始 JSON，避免重复轰炸上游）。
// ============================================================================

mod api;
mod error;
mod export;
mod http;
mod util;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use jxcg_api::{Banben, Cailiao, Chuchan, Fuwuqi, Jiyi, Peifang, Pinlei, Wupin};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use error::{Error, Result};
use export::{DataBundle, ExportReport, PeifangOut};
use http::HttpClient;

/// 五行石（1~6 级）物品 id：`wuxingshi.toml` 的手工样例会引用，必须随包带上。
const WUXINGSHI_ITEMS: [&str; 6] = [
    "5_24423", "5_24424", "5_24425", "5_24426", "5_24427", "5_24428",
];

const MAX_CONCURRENCY: usize = 6;

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
    if args.offline {
        println!("离线模式：只读 --cache 缓存，不联网");
    }

    let client = HttpClient::new(
        args.timeout_secs,
        args.cache.clone(),
        args.offline,
        args.refresh_cache,
    )?;

    // ---- 1) 配方目录（5 技艺） ----
    println!("\n[1/5] 拉取配方目录");
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

    // ---- 2) 配方详情（并发 + 进度） ----
    println!("\n[2/5] 拉取配方详情（共 {total} 条）");
    let (details, detail_failed) = fetch_details(&client, tasks, args.concurrency, started).await;

    // ---- 3) 物品（配方材料 + 产物 + 五行石） ----
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
    let mut item_ids: BTreeSet<String> = BTreeSet::new();
    item_ids.extend(material_ids.iter().cloned());
    item_ids.extend(product_ids.iter().cloned());
    item_ids.extend(WUXINGSHI_ITEMS.iter().map(|id| id.to_string()));
    println!(
        "\n[3/5] 拉取物品（共 {} 个：材料 {} / 产物 {}）",
        item_ids.len(),
        material_ids.len(),
        product_ids.len()
    );
    let item_id_list: Vec<String> = item_ids.iter().cloned().collect();
    let (items, item_missing) =
        fetch_items(&client, &item_id_list, args.concurrency, started).await;

    // ---- 4) NPC 商店价 + 服务器 ----
    println!("\n[4/5] 拉取 NPC 商店价与服务器列表");
    let craft_prices = api::fetch_craft_prices(&client).await?;
    println!("  商店价：{} 条", craft_prices.len());
    let fuwuqi = api::fetch_servers(&client).await?;
    println!("  服务器：{} 个", fuwuqi.len());

    // ---- 5) 组装 + 写盘 ----
    println!("\n[5/5] 生成 data/*.toml");
    let wupin = build_wupin(
        &item_ids,
        &items,
        &material_ids,
        &product_ids,
        &craft_prices,
    );
    let peifang = build_peifang(details);

    // 详情失败太多说明上游异常（或本地网络差），宁可不覆盖数据。
    if args.limit.is_none() && detail_failed * 10 > total {
        return Err(Error::Data(format!(
            "配方详情失败 {detail_failed}/{total}（超过 10%），为避免写入残缺数据已中止"
        )));
    }

    let bundle = DataBundle {
        generated_unix,
        banben: Banben {
            level: 130,
            season: 3,
            modified: util::modified_date(generated_unix),
            note: format!(
                "由 server_tools/jx3box-export 于 {} 从 JX3BOX 导出",
                util::date_iso_bj(generated_unix)
            ),
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
        ..ExportReport::default()
    };
    export::write_data_dir(&args.out, &bundle, &mut report)?;
    report.elapsed_ms = started.elapsed().as_millis();

    print_summary(&bundle, &report);
    Ok(())
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
    material_ids: &BTreeSet<String>,
    product_ids: &BTreeSet<String>,
    craft_prices: &BTreeMap<u32, i64>,
) -> Vec<Wupin> {
    let wuxingshi: BTreeSet<&str> = WUXINGSHI_ITEMS.into_iter().collect();
    let mut wupin = Vec::with_capacity(item_ids.len());
    for item_id in item_ids {
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
    };
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
            other => {
                return Err(Error::Data(format!("未知参数：{other}\n\n{}", usage())));
            }
        }
    }
    if args.offline && args.cache.is_none() {
        return Err(Error::Data("--offline 需要同时指定 --cache".into()));
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
    println!("配方 {} 条（{peifang_summary}）", bundle.peifang.len());
    println!(
        "物品 {} 条（{pinlei_summary}）；其中有 NPC 商店价 {} 条",
        bundle.wupin.len(),
        report.shangdian_count
    );
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
