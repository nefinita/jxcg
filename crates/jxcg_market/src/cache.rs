//! 本地行情缓存（用户数据，不进仓库）：`{数据目录}/hangqing/{服务器}.toml`。
//!
//! 目录布局沿用 PLAN §7：行情 `hangqing/{server}.toml`、快照 `hangqing/{server}/snapshots/*`、
//! 设置 `settings.toml`；本模块只负责行情主文件，快照由上层（Tauri）另行实现。
//!
//! 文件示例：
//!
//! ```toml
//! banben = 1
//! fuwuqi_id = "jiandanqinxin"
//! gengxin_shijian = 1794413423
//!
//! [[wupin]]
//! wupin_id = "5_30853"
//! gengxin_shijian = 1794413423
//! laiyuan = "jx3box"
//! shoudong = false
//!
//! [[wupin.guadan]]
//! shijian = 1794413423
//! shuliang = 24
//! danjia = 1885800
//! ```
//!
//! 注意：文件结构是独立于 `jxcg_api::JiageJilu` 的（字段顺序：标量在前、`guadan` 子表在后），
//! 这样才能序列化成合法 TOML；对外仍用 DTO，保持 Tauri/前端契约不变。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use jxcg_api::{Guadan, JiageJilu, Tong};
use serde::{Deserialize, Serialize};

use crate::error::{MarketError, Result};

/// 行情缓存子目录名（相对应用数据目录）。
pub const HANGQING_DIR: &str = "hangqing";

/// 缓存文件 schema 版本；结构不兼容变更时 +1。
pub const CACHE_BANBEN: u32 = 1;

/// 一个服务器的行情档案（内存形态）。
#[derive(Debug, Clone, Default)]
pub struct HangqingDangan {
    pub fuwuqi_id: String,
    /// 本文件最后刷新时间（秒）
    pub gengxin_shijian: i64,
    pub wupin: Vec<JiageJilu>,
}

impl HangqingDangan {
    pub fn new(fuwuqi_id: &str) -> Self {
        Self {
            fuwuqi_id: fuwuqi_id.to_string(),
            gengxin_shijian: 0,
            wupin: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.wupin.is_empty()
    }

    pub fn len(&self) -> usize {
        self.wupin.len()
    }

    /// 找单条物品行情。
    pub fn find(&self, wupin_id: &str) -> Option<&JiageJilu> {
        self.wupin.iter().find(|jilu| jilu.wupin_id == wupin_id)
    }

    /// 插入或替换单条行情（按 `wupin_id`）。
    pub fn upsert(&mut self, jilu: JiageJilu) {
        match self
            .wupin
            .iter_mut()
            .find(|existing| existing.wupin_id == jilu.wupin_id)
        {
            Some(existing) => *existing = jilu,
            None => self.wupin.push(jilu),
        }
    }

    /// 物品 id → 最低挂单价（铜）。缺价物品不在结果里。
    pub fn jiage_map(&self) -> BTreeMap<String, Tong> {
        jiage_map(&self.wupin)
    }

    /// 按物品 id（类型, 索引）排序，保证写盘稳定、用户 git diff 友好。
    pub fn sort(&mut self) {
        self.wupin
            .sort_by_key(|jilu| wupin_sort_key(&jilu.wupin_id));
    }
}

/// 行情目录：`{数据目录}/hangqing`。
pub fn hangqing_dir(dir: &Path) -> PathBuf {
    dir.join(HANGQING_DIR)
}

/// 缓存文件路径：`{数据目录}/hangqing/{服务器}.toml`。
///
/// 服务器 id 会做文件名清洗（去掉路径分隔符等），避免越权写到目录外。
pub fn cache_path(dir: &Path, fuwuqi_id: &str) -> PathBuf {
    hangqing_dir(dir).join(format!("{}.toml", sanitize_file_name(fuwuqi_id)))
}

fn sanitize_file_name(fuwuqi_id: &str) -> String {
    let clean: String = fuwuqi_id
        .chars()
        .map(|ch| {
            // 中文等 Unicode 字母数字保留（服务器名兜底时是中文 id）
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "unknown".to_string()
    } else {
        clean
    }
}

/// 单条行情里的最低挂单价（铜）；没有挂单返回 None。
pub fn zuidi_danjia(jilu: &JiageJilu) -> Option<Tong> {
    jilu.guadan
        .iter()
        .map(|guadan| guadan.danjia)
        .filter(|danjia| *danjia > 0)
        .min()
}

/// 多条行情 → 物品 id → 最低挂单价（供核心计算直接用；缺价物品不出现）。
pub fn jiage_map(prices: &[JiageJilu]) -> BTreeMap<String, Tong> {
    prices
        .iter()
        .filter_map(|jilu| zuidi_danjia(jilu).map(|danjia| (jilu.wupin_id.clone(), danjia)))
        .collect()
}

/// 物品 id `"{类型}_{索引}"` 的数值排序键。
pub fn wupin_sort_key(wupin_id: &str) -> (u32, u32) {
    let mut parts = wupin_id.split('_');
    let item_type = parts
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(u32::MAX);
    let index = parts
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(u32::MAX);
    (item_type, index)
}

// ============================================================================
// 文件读写
// ============================================================================

/// 文件形态：字段顺序 = 标量在前、`guadan` 子表在后（TOML 嵌套要求）。
#[derive(Debug, Serialize, Deserialize)]
struct DanganFile {
    #[serde(default = "default_banben")]
    banben: u32,
    fuwuqi_id: String,
    #[serde(default)]
    gengxin_shijian: i64,
    #[serde(default)]
    wupin: Vec<WupinShuju>,
}

fn default_banben() -> u32 {
    CACHE_BANBEN
}

#[derive(Debug, Serialize, Deserialize)]
struct WupinShuju {
    wupin_id: String,
    #[serde(default)]
    gengxin_shijian: i64,
    #[serde(default)]
    laiyuan: String,
    #[serde(default)]
    shoudong: bool,
    #[serde(default)]
    guadan: Vec<Guadan>,
}

/// 读某服的行情档案；文件不存在返回空档案（不算错误）。
pub fn load_dangan(dir: &Path, fuwuqi_id: &str) -> Result<HangqingDangan> {
    let path = cache_path(dir, fuwuqi_id);
    if !path.is_file() {
        return Ok(HangqingDangan::new(fuwuqi_id));
    }
    let text = std::fs::read_to_string(&path).map_err(|err| {
        MarketError::Cache(format!("读取行情缓存 {} 失败：{err}", path.display()))
    })?;
    let file: DanganFile = toml::from_str(&text).map_err(|err| {
        MarketError::Cache(format!("解析行情缓存 {} 失败：{err}", path.display()))
    })?;
    if file.banben > CACHE_BANBEN {
        return Err(MarketError::Cache(format!(
            "行情缓存版本过新（{} > 当前 {CACHE_BANBEN}）：{}",
            file.banben,
            path.display()
        )));
    }
    let mut dangan = HangqingDangan {
        fuwuqi_id: fuwuqi_id.to_string(),
        gengxin_shijian: file.gengxin_shijian,
        wupin: file
            .wupin
            .into_iter()
            .map(|row| JiageJilu {
                wupin_id: row.wupin_id,
                fuwuqi_id: fuwuqi_id.to_string(),
                guadan: row.guadan,
                gengxin_shijian: row.gengxin_shijian,
                laiyuan: if row.laiyuan.is_empty() {
                    "huancun".to_string()
                } else {
                    row.laiyuan
                },
                shoudong: row.shoudong,
            })
            .collect(),
    };
    dangan.sort();
    Ok(dangan)
}

/// 读某服的全部行情条目（Tauri `load_prices` 用；缺文件返回空）。
pub fn load_prices(dir: &Path, fuwuqi_id: &str) -> Result<Vec<JiageJilu>> {
    Ok(load_dangan(dir, fuwuqi_id)?.wupin)
}

/// 写某服行情档案（原子写：先写 `.tmp` 再 rename，避免半截文件）。
pub fn save_dangan(dir: &Path, dangan: &HangqingDangan) -> Result<()> {
    let path = cache_path(dir, &dangan.fuwuqi_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            MarketError::Cache(format!("创建行情目录 {} 失败：{err}", parent.display()))
        })?;
    }

    let mut wupin = dangan.wupin.clone();
    wupin.sort_by_key(|jilu| wupin_sort_key(&jilu.wupin_id));
    let file = DanganFile {
        banben: CACHE_BANBEN,
        fuwuqi_id: dangan.fuwuqi_id.clone(),
        gengxin_shijian: dangan.gengxin_shijian,
        wupin: wupin
            .into_iter()
            .map(|jilu| WupinShuju {
                wupin_id: jilu.wupin_id,
                gengxin_shijian: jilu.gengxin_shijian,
                laiyuan: jilu.laiyuan,
                shoudong: jilu.shoudong,
                guadan: jilu.guadan,
            })
            .collect(),
    };
    let text = toml::to_string_pretty(&file)
        .map_err(|err| MarketError::Cache(format!("序列化行情缓存失败：{err}")))?;
    let header = format!(
        "# JXCG 行情缓存（用户数据，勿手改；服务器 {}）\n\n",
        dangan.fuwuqi_id
    );

    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, format!("{header}{text}")).map_err(|err| {
        MarketError::Cache(format!("写入行情缓存 {} 失败：{err}", tmp_path.display()))
    })?;
    std::fs::rename(&tmp_path, &path).map_err(|err| {
        MarketError::Cache(format!("提交行情缓存 {} 失败：{err}", path.display()))
    })?;
    Ok(())
}

/// 手动覆盖一条物品价格（Tauri `upsert_price` 用）。
///
/// 写成 `shoudong = true` 的记录（来源 `shoudong`），刷新行情时**不会被自动价覆盖**
/// （PLAN D2：手动覆盖优先于自动行情）。
pub fn upsert_price(dir: &Path, fuwuqi_id: &str, wupin_id: &str, danjia: Tong) -> Result<()> {
    let mut dangan = load_dangan(dir, fuwuqi_id)?;
    let now = crate::now_unix();
    dangan.gengxin_shijian = now;
    dangan.upsert(JiageJilu {
        wupin_id: wupin_id.to_string(),
        fuwuqi_id: fuwuqi_id.to_string(),
        // 手动价没有真实挂单明细，用一个占位挂单承载单价（数量 1）
        guadan: vec![Guadan {
            shijian: now,
            shuliang: 1,
            danjia,
        }],
        gengxin_shijian: now,
        laiyuan: "shoudong".to_string(),
        shoudong: true,
    });
    save_dangan(dir, &dangan)
}

/// 写某服的全部行情条目（Tauri `save_prices` 用）。
pub fn save_prices(dir: &Path, fuwuqi_id: &str, prices: &[JiageJilu]) -> Result<()> {
    let gengxin_shijian = prices
        .iter()
        .map(|jilu| jilu.gengxin_shijian)
        .max()
        .unwrap_or(0);
    let dangan = HangqingDangan {
        fuwuqi_id: fuwuqi_id.to_string(),
        gengxin_shijian,
        wupin: prices.to_vec(),
    };
    save_dangan(dir, &dangan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// 工作区 `.cache/` 下的临时目录（沙箱 HOME 只读，不能用系统临时目录）。
    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.cache/market-test")
            .join(format!("{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("创建临时目录失败");
        dir
    }

    fn jilu(wupin_id: &str, prices: &[i64], gengxin: i64) -> JiageJilu {
        JiageJilu {
            wupin_id: wupin_id.to_string(),
            fuwuqi_id: "jiandanqinxin".to_string(),
            guadan: prices
                .iter()
                .enumerate()
                .map(|(index, danjia)| Guadan {
                    shijian: gengxin + index as i64,
                    shuliang: 10,
                    danjia: *danjia,
                })
                .collect(),
            gengxin_shijian: gengxin,
            laiyuan: "jx3box".to_string(),
            shoudong: false,
        }
    }

    #[test]
    fn missing_file_is_empty() {
        let dir = temp_dir("missing");
        let dangan = load_dangan(&dir, "jiandanqinxin").expect("缺文件应返回空档案");
        assert!(dangan.is_empty());
        assert_eq!(dangan.fuwuqi_id, "jiandanqinxin");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        let prices = vec![
            jilu("5_30853", &[1_885_800, 2_000_000], 1_794_413_423),
            jilu("5_30845", &[], 1_794_413_500),
        ];
        save_prices(&dir, "jiandanqinxin", &prices).expect("写缓存失败");

        // 文件应是 `hangqing/{服务器}.toml`
        let path = cache_path(&dir, "jiandanqinxin");
        assert!(path.is_file(), "缓存文件不存在：{}", path.display());
        let text = std::fs::read_to_string(&path).expect("读缓存失败");
        assert!(text.contains("[[wupin]]"));
        assert!(text.contains("[[wupin.guadan]]"));
        assert!(text.contains("laiyuan = \"jx3box\""));

        let loaded = load_prices(&dir, "jiandanqinxin").expect("读缓存失败");
        assert_eq!(loaded.len(), 2);
        // 写盘按物品 id 数值排序（5_30845 在 5_30853 前）
        assert_eq!(loaded[0].wupin_id, "5_30845");
        let first = loaded
            .iter()
            .find(|jilu| jilu.wupin_id == "5_30853")
            .expect("5_30853 应在缓存里");
        assert_eq!(first.guadan.len(), 2);
        assert_eq!(first.fuwuqi_id, "jiandanqinxin");
        // 新条目覆盖旧文件（不是追加）
        save_prices(
            &dir,
            "jiandanqinxin",
            &[jilu("5_30853", &[123], 1_794_413_999)],
        )
        .expect("覆盖写失败");
        let loaded = load_prices(&dir, "jiandanqinxin").expect("读缓存失败");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].guadan[0].danjia, 123);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_file_is_reported() {
        let dir = temp_dir("corrupt");
        let path = cache_path(&dir, "bad");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("建目录失败");
        std::fs::write(&path, "这不是 toml [[[").expect("写坏文件失败");
        let err = load_dangan(&dir, "bad").expect_err("坏文件应报错");
        assert!(matches!(err, MarketError::Cache(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_path_is_sanitized() {
        let dir = Path::new("/tmp/data");
        let path = cache_path(dir, "../../etc/passwd");
        assert_eq!(path.parent(), Some(hangqing_dir(dir).as_path()));
        assert!(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .ends_with(".toml")
        );
        // 中文 id（服务器 pinyin 表未收录时的兜底）也应可作文件名
        let path = cache_path(dir, "剑胆琴心");
        assert!(path.file_name().is_some());
    }

    #[test]
    fn upsert_price_marks_manual_override() {
        let dir = temp_dir("shoudong");
        save_prices(&dir, "jiandanqinxin", &[jilu("5_30853", &[100], 1)]).expect("写缓存失败");
        upsert_price(&dir, "jiandanqinxin", "5_30853", 9_999).expect("手动覆盖失败");
        upsert_price(&dir, "jiandanqinxin", "5_99999", 4_321).expect("手动新增失败");
        let prices = load_prices(&dir, "jiandanqinxin").expect("读缓存失败");
        let overridden = prices
            .iter()
            .find(|jilu| jilu.wupin_id == "5_30853")
            .expect("被覆盖的物品应在缓存里");
        assert!(overridden.shoudong);
        assert_eq!(overridden.laiyuan, "shoudong");
        assert_eq!(zuidi_danjia(overridden), Some(9_999));
        assert_eq!(prices.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jiage_map_uses_lowest_listing() {
        let prices = vec![jilu("5_1", &[500, 300, 400], 0), jilu("5_2", &[], 0)];
        let map = jiage_map(&prices);
        assert_eq!(map.get("5_1"), Some(&300));
        assert!(!map.contains_key("5_2"));
    }

    #[test]
    fn dangan_upsert_and_sort() {
        let mut dangan = HangqingDangan::new("x");
        dangan.upsert(jilu("5_10", &[1], 0));
        dangan.upsert(jilu("5_2", &[2], 0));
        dangan.upsert(jilu("5_10", &[9], 0));
        assert_eq!(dangan.len(), 2);
        assert_eq!(dangan.find("5_10").map(|j| j.guadan[0].danjia), Some(9));
        dangan.sort();
        assert_eq!(dangan.wupin[0].wupin_id, "5_2");
        assert_eq!(dangan.wupin[1].wupin_id, "5_10");
    }
}
