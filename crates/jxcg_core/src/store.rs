//! 随包数据读取：data/{catalog,wupin,peifang,wuxingshi,fuwuqi,values}.toml。
//!
//! 数据目录解析顺序（AGENTS.md / PLAN.md §7）：
//! 1. 环境变量 `JXCG_DATA_DIR`（显式指定；**设置了但目录无效时报错，不静默回落**）
//! 2. 可执行文件同目录下的 `data/`
//! 3. debug 构建下回退到仓库内 `data/`（开发期方便，release 不启用）
//!
//! 错误信息统一中文并带文件名/路径；结构校验（重复 id、产物 min>max）在装载后执行。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::{JxcgError, Result};
use jxcg_api::{Banben, Catalog, Fuwuqi, Peifang, Wupin, Zhiji, Zhuanhuan};
use serde::de::DeserializeOwned;

/// 随包数据文件名（与 data/ 目录约定一致）。
pub const CATALOG_FILE: &str = "catalog.toml";
pub const WUPIN_FILE: &str = "wupin.toml";
pub const PEIFANG_FILE: &str = "peifang.toml";
pub const WUXINGSHI_FILE: &str = "wuxingshi.toml";
pub const FUWUQI_FILE: &str = "fuwuqi.toml";
pub const VALUES_FILE: &str = "values.toml";

#[derive(serde::Deserialize)]
struct CatalogFile {
    banben: Banben,
}

#[derive(serde::Deserialize, Default)]
struct WupinFile {
    #[serde(default)]
    wupin: Vec<Wupin>,
}

#[derive(serde::Deserialize, Default)]
struct PeifangFile {
    #[serde(default)]
    peifang: Vec<Peifang>,
}

#[derive(serde::Deserialize, Default)]
struct WuxingshiFile {
    #[serde(default)]
    zhuanhuan: Vec<Zhuanhuan>,
}

#[derive(serde::Deserialize, Default)]
struct FuwuqiFile {
    #[serde(default)]
    fuwuqi: Vec<Fuwuqi>,
}

#[derive(serde::Deserialize, Default)]
struct ValuesFile {
    #[serde(default)]
    zhiji: Vec<Zhiji>,
}

fn read_toml<T: DeserializeOwned>(dir: &Path, name: &str) -> Result<T> {
    let path = dir.join(name);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(JxcgError::Io(format!(
                "缺少数据文件：{}（可用 JXCG_DATA_DIR 指定数据目录）",
                path.display()
            )));
        }
        Err(err) => {
            return Err(JxcgError::Io(format!(
                "读取数据文件 {} 失败：{err}",
                path.display()
            )));
        }
    };
    toml::from_str(&text)
        .map_err(|err| JxcgError::Toml(format!("解析数据文件 {} 失败：{err}", path.display())))
}

/// 从指定目录加载完整目录数据（含结构校验）。
pub fn load_catalog(dir: &Path) -> Result<Catalog> {
    let catalog_file: CatalogFile = read_toml(dir, CATALOG_FILE)?;
    let wupin: WupinFile = read_toml(dir, WUPIN_FILE)?;
    let peifang: PeifangFile = read_toml(dir, PEIFANG_FILE)?;
    let wuxingshi: WuxingshiFile = read_toml(dir, WUXINGSHI_FILE)?;
    let fuwuqi: FuwuqiFile = read_toml(dir, FUWUQI_FILE)?;
    let values: ValuesFile = read_toml(dir, VALUES_FILE)?;
    let catalog = Catalog {
        banben: catalog_file.banben,
        wupin: wupin.wupin,
        peifang: peifang.peifang,
        wuxingshi: wuxingshi.zhuanhuan,
        fuwuqi: fuwuqi.fuwuqi,
        zhiji: values.zhiji,
    };
    validate_catalog(&catalog)?;
    Ok(catalog)
}

/// 结构校验：重复 id、产物槽 min > max 等会让计算结果不可信的问题。
///
/// 只校验「结构性错误」；缺价、缺物品名等属于业务完整度，不在此报错。
pub fn validate_catalog(catalog: &Catalog) -> Result<()> {
    check_duplicate("物品", catalog.wupin.iter().map(|w| w.id.as_str()))?;
    check_duplicate("配方", catalog.peifang.iter().map(|p| p.id.as_str()))?;
    check_duplicate(
        "五行石转换",
        catalog.wuxingshi.iter().map(|z| z.id.as_str()),
    )?;
    check_duplicate("服务器", catalog.fuwuqi.iter().map(|f| f.id.as_str()))?;
    check_duplicate("值集", catalog.zhiji.iter().map(|z| z.id.as_str()))?;
    for peifang in &catalog.peifang {
        for slot in &peifang.chuchan {
            if slot.min > slot.max {
                return Err(JxcgError::Data(format!(
                    "配方「{}」（{}）的产物槽 min > max（{} > {}）",
                    peifang.mingcheng, peifang.id, slot.min, slot.max
                )));
            }
        }
    }
    Ok(())
}

fn check_duplicate<'a>(what: &str, ids: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen: HashSet<&str> = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(JxcgError::Data(format!(
                "{what} id 重复：{id}（请检查 data/ TOML）"
            )));
        }
    }
    Ok(())
}

/// 解析数据目录（宽松版，找不到返回 None；保留给只做探测的调用方）。
pub fn resolve_data_dir() -> Option<PathBuf> {
    resolve_data_dir_strict().ok()
}

/// 解析数据目录（严格版，失败给中文错误）。
///
/// `JXCG_DATA_DIR` 一旦设置就优先，且**必须指向存在的目录**，否则报错而不是悄悄回落。
pub fn resolve_data_dir_strict() -> Result<PathBuf> {
    let env_value = std::env::var("JXCG_DATA_DIR").ok();
    let exe_parent = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    #[cfg(debug_assertions)]
    let debug_fallback = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    #[cfg(not(debug_assertions))]
    let debug_fallback: Option<PathBuf> = None;
    resolve_data_dir_impl(
        env_value.as_deref(),
        exe_parent.as_deref(),
        debug_fallback.as_deref(),
    )
}

/// 数据目录解析的纯逻辑（便于单测注入环境变量/可执行文件路径，不受进程环境影响）。
///
/// 优先级：有效 `JXCG_DATA_DIR` > exe 同目录 `data/` > debug 仓库 `data/`；
/// `env_override` 为 `Some` 但目录不存在时直接报错。
fn resolve_data_dir_impl(
    env_override: Option<&str>,
    exe_parent: Option<&Path>,
    debug_fallback: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(raw) = env_override {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.is_dir() {
                return Ok(path);
            }
            return Err(JxcgError::Data(format!(
                "JXCG_DATA_DIR 指向的目录不存在或不是目录：{}",
                path.display()
            )));
        }
    }
    if let Some(parent) = exe_parent {
        let candidate = parent.join("data");
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    if let Some(fallback) = debug_fallback
        && fallback.is_dir()
    {
        return Ok(fallback.to_path_buf());
    }
    Err(JxcgError::Data(
        "找不到 data/ 目录（可用 JXCG_DATA_DIR 指定数据目录）".into(),
    ))
}

/// 解析并加载默认数据目录（解析失败给中文错误）。
pub fn load_from_default_dir() -> Result<(PathBuf, Catalog)> {
    let dir = resolve_data_dir_strict()?;
    let catalog = load_catalog(&dir)?;
    Ok((dir, catalog))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);

    /// 在 target/ 下建独立临时目录（沙箱里 HOME 可能只读，不放系统临时目录）。
    fn test_dir(tag: &str) -> PathBuf {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/jxcg-test-data")
            .join(format!("{tag}-{}-{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建测试目录");
        dir
    }

    /// 写一套最小可加载数据；`wupin` 用调用方内容，其余文件给合法骨架。
    fn write_min_data(dir: &Path, wupin: &str) {
        std::fs::write(
            dir.join(CATALOG_FILE),
            "[banben]\nlevel = 130\nseason = 3\nmodified = 20261112\nnote = \"test\"\n",
        )
        .expect("写 catalog.toml");
        std::fs::write(dir.join(WUPIN_FILE), wupin).expect("写 wupin.toml");
        std::fs::write(
            dir.join(PEIFANG_FILE),
            "[[peifang]]\nid = \"cooking_1\"\nmingcheng = \"测试\"\njiyi = \"pengren\"\ndengji = 10\njingli = 5\n\n[[peifang.cailiao]]\nwupin_id = \"5_1\"\nshuliang = 1\n\n[[peifang.chuchan]]\nwupin_id = \"5_2\"\nmin = 1\nmax = 1\nquanzhong = 1048576\n",
        )
        .expect("写 peifang.toml");
        std::fs::write(
            dir.join(WUXINGSHI_FILE),
            "[[zhuanhuan]]\nid = \"w1\"\nmingcheng = \"测试转换\"\ncong = \"5_1\"\ndao = \"5_2\"\nxuyao = 3\njingli = 10\njinbi = 100\nchenggonglv_bp = 5000\n",
        )
        .expect("写 wuxingshi.toml");
        std::fs::write(
            dir.join(FUWUQI_FILE),
            "[[fuwuqi]]\nid = \"jiandanqinxin\"\nmingcheng = \"剑胆琴心\"\n",
        )
        .expect("写 fuwuqi.toml");
        std::fs::write(
            dir.join(VALUES_FILE),
            "[[zhiji]]\nid = \"live-130\"\nmingcheng = \"正式服\"\n\n[zhiji.shouxufei]\nshuilv_bp = 500\nquzheng = \"half_up\"\nbaoguanfei = 8000\n",
        )
        .expect("写 values.toml");
    }

    const VALID_WUPIN: &str = "[[wupin]]\nid = \"5_1\"\nmingcheng = \"材料\"\npinlei = \"cailiao\"\nshangdianjia = 10\n\n[[wupin]]\nid = \"5_2\"\nmingcheng = \"成品\"\npinlei = \"chengpin\"\n";

    #[test]
    fn load_repo_data() {
        // 随包数据必须能被加载（market-data 重新导出后仍须通过）
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let catalog = load_catalog(&dir).expect("data/ 应能加载");
        assert!(catalog.banben.level > 0, "数据版本 level 应 > 0");
        assert!(!catalog.wupin.is_empty(), "物品表不应为空");
        assert!(!catalog.peifang.is_empty(), "配方表不应为空");
        assert!(!catalog.fuwuqi.is_empty(), "服务器表不应为空");
        assert!(!catalog.zhiji.is_empty(), "值集不应为空");
        assert!(
            catalog.zhiji.iter().all(|z| {
                z.shouxufei.shuilv_bp >= 0
                    && z.shouxufei.shuilv_bp < 10_000
                    && z.shouxufei.baoguanfei >= 0
            }),
            "值集费率/保管费应在合法范围"
        );
    }

    #[test]
    fn load_minimal_data() {
        let dir = test_dir("minimal");
        write_min_data(&dir, VALID_WUPIN);
        let catalog = load_catalog(&dir).expect("最小数据应能加载");
        assert_eq!(catalog.banben.level, 130);
        assert_eq!(catalog.wupin.len(), 2);
        assert_eq!(catalog.peifang.len(), 1);
        assert_eq!(catalog.wuxingshi.len(), 1);
        assert_eq!(catalog.fuwuqi.len(), 1);
        assert_eq!(catalog.zhiji.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_error_is_chinese() {
        let dir = test_dir("missing");
        write_min_data(&dir, VALID_WUPIN);
        // 删掉 peifang.toml
        std::fs::remove_file(dir.join(PEIFANG_FILE)).expect("删 peifang.toml");
        let err = load_catalog(&dir).expect_err("缺文件应报错");
        let text = err.to_string();
        assert!(
            text.contains("缺少数据文件"),
            "错误应中文提示缺文件：{text}"
        );
        assert!(text.contains(PEIFANG_FILE), "错误应带文件名：{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_field_error_is_chinese() {
        let dir = test_dir("badfield");
        let bad = VALID_WUPIN.replace(
            "id = \"5_1\"\nmingcheng = \"材料\"",
            "id = \"5_1\"\nmingcheng = \"材料\"\ndui = \"不是数字\"",
        );
        write_min_data(&dir, &bad);
        let err = load_catalog(&dir).expect_err("坏字段应报错");
        let text = err.to_string();
        assert!(text.contains("解析数据文件"), "错误应中文提示：{text}");
        assert!(text.contains(WUPIN_FILE), "错误应带文件名：{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_id_rejected() {
        let dir = test_dir("dup");
        let dup = format!(
            "{VALID_WUPIN}\n[[wupin]]\nid = \"5_1\"\nmingcheng = \"重复\"\npinlei = \"cailiao\"\n"
        );
        write_min_data(&dir, &dup);
        let err = load_catalog(&dir).expect_err("重复 id 应报错");
        assert!(err.to_string().contains("id 重复"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn min_max_rejected() {
        let dir = test_dir("minmax");
        write_min_data(&dir, VALID_WUPIN);
        std::fs::write(
            dir.join(PEIFANG_FILE),
            "[[peifang]]\nid = \"cooking_1\"\nmingcheng = \"测试\"\njiyi = \"pengren\"\ndengji = 10\njingli = 5\n\n[[peifang.chuchan]]\nwupin_id = \"5_2\"\nmin = 3\nmax = 1\nquanzhong = 1048576\n",
        )
        .expect("写坏 peifang.toml");
        let err = load_catalog(&dir).expect_err("min>max 应报错");
        assert!(err.to_string().contains("min > max"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_priority() {
        let env_dir = test_dir("env");
        let exe_dir = test_dir("exe");
        let debug_dir = test_dir("debug");
        write_min_data(&env_dir, VALID_WUPIN);
        let exe_data = exe_dir.join("data");
        std::fs::create_dir_all(&exe_data).expect("建 exe/data");
        let debug_data = debug_dir.join("data");
        std::fs::create_dir_all(&debug_data).expect("建 debug/data");

        // 1) 环境变量优先
        let got = resolve_data_dir_impl(
            Some(env_dir.to_str().expect("utf8")),
            Some(&exe_dir),
            Some(&debug_data),
        )
        .expect("env 应优先");
        assert_eq!(got, env_dir);
        // 2) 无环境变量 → exe 同目录 data/
        let got = resolve_data_dir_impl(None, Some(&exe_dir), Some(&debug_data))
            .expect("应回退 exe/data");
        assert_eq!(got, exe_data);
        // 3) exe 也没有 → debug 仓库 data/
        let empty_exe = test_dir("noexe");
        let got = resolve_data_dir_impl(None, Some(&empty_exe), Some(&debug_data))
            .expect("应回退 debug data");
        assert_eq!(got, debug_data);
        // 4) 全部没有 → 中文错误
        let err = resolve_data_dir_impl(None, Some(&empty_exe), None).expect_err("应报错");
        assert!(err.to_string().contains("找不到 data/ 目录"), "{err}");
        // 5) 环境变量指向不存在路径 → 报错，不静默回落
        let err = resolve_data_dir_impl(
            Some("/definitely/not/exists/jxcg"),
            Some(&exe_dir),
            Some(&debug_data),
        )
        .expect_err("无效 env 应报错");
        let text = err.to_string();
        assert!(text.contains("JXCG_DATA_DIR"), "{text}");
        // 6) 环境变量为空串 → 视为未设置
        let got = resolve_data_dir_impl(Some("   "), Some(&exe_dir), Some(&debug_data))
            .expect("空 env 应视为未设置");
        assert_eq!(got, exe_data);

        for dir in [env_dir, exe_dir, debug_dir, empty_exe] {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn env_override_end_to_end() {
        let dir = test_dir("env-e2e");
        write_min_data(&dir, VALID_WUPIN);
        let old = std::env::var_os("JXCG_DATA_DIR");
        // SAFETY: 本测试是唯一读写该环境变量的单元测试，其他测试不调用 resolve_data_dir_strict。
        unsafe {
            std::env::set_var("JXCG_DATA_DIR", &dir);
        }
        let resolved = resolve_data_dir_strict();
        let loaded = load_from_default_dir();
        match old {
            Some(v) => unsafe { std::env::set_var("JXCG_DATA_DIR", v) },
            None => unsafe { std::env::remove_var("JXCG_DATA_DIR") },
        }
        let resolved = resolved.expect("JXCG_DATA_DIR 应覆盖成功");
        assert_eq!(resolved, dir);
        let (loaded_dir, catalog) = loaded.expect("默认目录装载应走 env");
        assert_eq!(loaded_dir, dir);
        assert_eq!(catalog.wupin.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
