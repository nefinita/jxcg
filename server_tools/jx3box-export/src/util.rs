//! 通用小工具：日期、服务器 pinyin id、排序键。

/// 当前 Unix 时间戳（秒）。
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Unix 时间戳 → (年, 月, 日)。
///
/// Howard Hinnant 的 civil_from_days 算法（只依赖标准库，避免为日期加依赖）。
pub fn ymd_from_unix(ts: i64) -> (i64, u32, u32) {
    let days = ts.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    if m <= 2 {
        y += 1;
    }
    (y, m as u32, d as u32)
}

/// Unix 时间戳 → `YYYYMMDD`（catalog.toml 的 `banben.modified` 口径）。
pub fn modified_yyyymmdd(ts: i64) -> u32 {
    let (y, m, d) = ymd_from_unix(ts);
    (y * 10_000 + m as i64 * 100 + d as i64) as u32
}

/// Unix 时间戳 → `YYYY-MM-DD`（写进注释/数据 README）。
pub fn date_iso(ts: i64) -> String {
    let (y, m, d) = ymd_from_unix(ts);
    format!("{y:04}-{m:02}-{d:02}")
}

/// 北京时间相对 UTC 的偏移（秒）。
///
/// 游戏数据按国内时间维护，导出日期统一记北京时间（UTC+8），
/// 避免 UTC 凌晨导出时日期比本地少一天。
pub const BEIJING_OFFSET_SECS: i64 = 8 * 3600;

/// Unix 时间戳 → 北京时间的 `YYYYMMDD`。
pub fn modified_date(ts: i64) -> u32 {
    modified_yyyymmdd(ts.saturating_add(BEIJING_OFFSET_SECS))
}

/// Unix 时间戳 → 北京时间的 `YYYY-MM-DD`。
pub fn date_iso_bj(ts: i64) -> String {
    date_iso(ts.saturating_add(BEIJING_OFFSET_SECS))
}

/// 物品 id `"{类型}_{索引}"` 的数值排序键（类型, 索引）。
pub fn item_sort_key(item_id: &str) -> (u32, u32) {
    let mut parts = item_id.split('_');
    let typ = parts
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(u32::MAX);
    let index = parts
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or(u32::MAX);
    (typ, index)
}

/// 服务器中文名 → 稳定的 pinyin id（用于文件名/设置项）。
///
/// 上游没有提供 pinyin，本表手工维护；未收录的新服回退为中文名（仍是合法 id），
/// 并在导出日志里提示补表。
pub fn pinyin_id(mingcheng: &str) -> Option<&'static str> {
    const MAP: &[(&str, &str)] = &[
        ("蝶恋花", "dielianhua"),
        ("龙争虎斗", "longzhenghudou"),
        ("长安城", "changancheng"),
        ("幽月轮", "youyuelun"),
        ("斗转星移", "douzhuanxingyi"),
        ("剑胆琴心", "jiandanqinxin"),
        ("乾坤一掷", "qiankunyizhi"),
        ("唯我独尊", "weiwoduzun"),
        ("梦江南", "mengjiangnan"),
        ("横刀断浪", "hengdaoduanlang"),
        ("绝代天骄", "juedaitianjiao"),
        ("天鹅坪", "tianeiping"),
        ("破阵子", "pozhenzi"),
        ("飞龙在天", "feilongzaitian"),
        ("九天", "jiutian"),
        ("眉间雪", "meijianxue"),
        ("山海相逢", "shanhaixiangfeng"),
        ("缘起稻香", "yuanqidaoxiang"),
        ("天宝盛世", "tianbaoshengshi"),
        ("剑啸江湖", "jianxiaojianghu"),
        ("共結來緣", "gongjielaiyuan"),
        ("傲血戰意", "aoxuezhenyi"),
        ("巔峰再起", "dianfengzaiqi"),
        ("江海雲夢", "jianghaiyunmeng"),
    ];
    MAP.iter()
        .find(|(name, _)| *name == mingcheng)
        .map(|(_, id)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ymd_known_dates() {
        // 2026-11-12 00:00:00 UTC
        assert_eq!(ymd_from_unix(1_794_441_600), (2026, 11, 12));
        assert_eq!(date_iso(1_794_441_600), "2026-11-12");
        assert_eq!(modified_yyyymmdd(1_794_441_600), 20_261_112);
        // 1970-01-01
        assert_eq!(ymd_from_unix(0), (1970, 1, 1));
        // 北京时间口径：2026-11-11T16:00Z = 2026-11-12 00:00 北京
        assert_eq!(modified_date(1_794_412_800), 20_261_112);
        assert_eq!(date_iso_bj(1_794_412_800), "2026-11-12");
    }

    #[test]
    fn item_sort_key_parses() {
        assert_eq!(item_sort_key("5_30853"), (5, 30_853));
        assert_eq!(item_sort_key("10_7"), (10, 7));
        assert!(item_sort_key("bad") > (5, 30_853));
    }

    #[test]
    fn pinyin_known_server() {
        assert_eq!(pinyin_id("剑胆琴心"), Some("jiandanqinxin"));
        assert_eq!(pinyin_id("不存在的服"), None);
    }
}
