// ============================================================================
// jxcg_api —— 前后端共享 DTO 契约（单一来源，零业务逻辑）
//
// 约定（沿用 jpcg 风格）：
// - 金额一律是「铜」为单位的 i64（Tong），禁止用浮点表示金额；
// - 费率用万分比（shuilv_bp：500 = 5%），成功率用万分比（chenggonglv_bp）；
// - 产出概率权重分母固定 2^20 = 1_048_576（QUANZHONG_FENMU）；
// - 拼音键名 + 中文注释；这里的字段变更会同时影响 Tauri 命令与前端类型。
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 金额：铜。1 金 = 100 银 = 10_000 铜；1 砖 = 10_000 金 = 100_000_000 铜。
pub type Tong = i64;

/// 产出概率权重分母（游戏数据口径，2^20）。
pub const QUANZHONG_FENMU: u32 = 1_048_576;

// ============================================================================
// 枚举
// ============================================================================

/// 生活技艺类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Jiyi {
    /// 烹饪
    Pengren,
    /// 制药（医术）
    Zhiyao,
    /// 缝纫
    Fengren,
    /// 铸造（锻造）
    Zhuzhao,
    /// 梓匠（家具）
    Zijiang,
}

impl Jiyi {
    pub const ALL: [Jiyi; 5] = [
        Jiyi::Pengren,
        Jiyi::Zhiyao,
        Jiyi::Fengren,
        Jiyi::Zhuzhao,
        Jiyi::Zijiang,
    ];

    /// 稳定的协议 id（与 TOML `jiyi` 字段一致）。
    pub fn id(self) -> &'static str {
        match self {
            Jiyi::Pengren => "pengren",
            Jiyi::Zhiyao => "zhiyao",
            Jiyi::Fengren => "fengren",
            Jiyi::Zhuzhao => "zhuzhao",
            Jiyi::Zijiang => "zijiang",
        }
    }

    /// 界面显示名。
    pub fn mingcheng(self) -> &'static str {
        match self {
            Jiyi::Pengren => "烹饪",
            Jiyi::Zhiyao => "制药",
            Jiyi::Fengren => "缝纫",
            Jiyi::Zhuzhao => "铸造",
            Jiyi::Zijiang => "梓匠",
        }
    }

    pub fn from_id(id: &str) -> Option<Jiyi> {
        Jiyi::ALL.into_iter().find(|j| j.id() == id)
    }
}

/// 物品类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pinlei {
    /// 材料
    Cailiao,
    /// 成品
    Chengpin,
    /// 消耗品（食物/药品等）
    Xiaohao,
    /// 五行石
    Wuxingshi,
}

/// 手续费取整方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quzheng {
    /// 四舍五入（默认，社区口径）
    HalfUp,
    /// 向下取整
    Floor,
    /// 向上取整
    Ceil,
}

/// 排行指标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaihangZhibiao {
    /// 单次净利润（铜）
    Lirun,
    /// 每点精力净利润（铜）
    MeiJingli,
    /// 利润率（万分比）
    Lirunlv,
}

// ============================================================================
// 静态数据（随包 TOML）
// ============================================================================

/// 数据版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Banben {
    /// 等级（当前正式服 130）
    pub level: u32,
    /// 赛季序号
    pub season: u32,
    /// 数据修订日期 YYYYMMDD
    pub modified: u32,
    /// 备注
    #[serde(default)]
    pub note: String,
}

/// 物品。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wupin {
    /// 游戏物品 id，形如 `5_30824`
    pub id: String,
    pub mingcheng: String,
    pub pinlei: Pinlei,
    /// 一组堆叠数量（按组购买用）
    #[serde(default)]
    pub dui: Option<u32>,
    /// 商店价（铜）；来自 JX3BOX craft/price，无则缺省
    #[serde(default)]
    pub shangdianjia: Option<Tong>,
    /// 数据来源标识
    #[serde(default)]
    pub laiyuan: Option<String>,
}

/// 配方材料需求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cailiao {
    pub wupin_id: String,
    pub shuliang: u32,
}

/// 配方产物槽（8 槽之一）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chuchan {
    pub wupin_id: String,
    pub min: u32,
    pub max: u32,
    /// 概率权重，分母 QUANZHONG_FENMU
    #[serde(default)]
    pub quanzhong: u32,
}

/// 生活技艺配方。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peifang {
    /// 形如 `cooking_437`
    pub id: String,
    pub mingcheng: String,
    pub jiyi: Jiyi,
    /// 配方等级要求（游戏内 RequireLevel）
    pub dengji: u32,
    /// 精力消耗（CostVigor）
    pub jingli: u32,
    /// 固定金币消耗（铜）
    #[serde(default)]
    pub zhizuofei: Tong,
    #[serde(default)]
    pub cailiao: Vec<Cailiao>,
    #[serde(default)]
    pub chuchan: Vec<Chuchan>,
    /// 数据来源标识
    #[serde(default)]
    pub laiyuan: Option<String>,
}

/// 五行石转换（精炼）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zhuanhuan {
    pub id: String,
    pub mingcheng: String,
    /// 低阶石物品 id
    pub cong: String,
    /// 高阶石物品 id
    pub dao: String,
    /// 每次投入低阶石数量
    pub xuyao: u32,
    /// 精力消耗
    pub jingli: u32,
    /// 金币消耗（铜）
    #[serde(default)]
    pub jinbi: Tong,
    /// 成功率，万分比（4878 = 48.78%）
    pub chenggonglv_bp: i64,
    #[serde(default)]
    pub laiyuan: Option<String>,
}

/// 服务器。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fuwuqi {
    pub id: String,
    pub mingcheng: String,
    /// 大区，如「电信区」「双线区」「无界区」
    #[serde(default)]
    pub daju: Option<String>,
}

/// 手续费值集。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shouxufei {
    /// 万分比，500 = 5%
    pub shuilv_bp: i64,
    pub quzheng: Quzheng,
    /// 保管费（铜 / 24h）
    pub baoguanfei: Tong,
}

impl Default for Shouxufei {
    fn default() -> Self {
        Self {
            shuilv_bp: 500,
            quzheng: Quzheng::HalfUp,
            baoguanfei: 8_000,
        }
    }
}

/// 值集（费率/取整/保管费快照，可随数据通道下发）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zhiji {
    pub id: String,
    pub mingcheng: String,
    #[serde(default)]
    pub note: String,
    pub shouxufei: Shouxufei,
}

/// 随包完整目录（core 读 TOML 后产出）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Catalog {
    pub banben: Banben,
    pub wupin: Vec<Wupin>,
    pub peifang: Vec<Peifang>,
    pub wuxingshi: Vec<Zhuanhuan>,
    pub fuwuqi: Vec<Fuwuqi>,
    pub zhiji: Vec<Zhiji>,
}

// ============================================================================
// 行情
// ============================================================================

/// 交易行挂单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guadan {
    /// 上架时间戳（秒）
    pub shijian: i64,
    /// 数量
    pub shuliang: u32,
    /// 单价（铜/件）
    pub danjia: Tong,
}

/// 单个物品在某服的价格记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JiageJilu {
    pub wupin_id: String,
    pub fuwuqi_id: String,
    #[serde(default)]
    pub guadan: Vec<Guadan>,
    /// 最后更新时间戳（秒）
    #[serde(default)]
    pub gengxin_shijian: i64,
    /// jx3box / shoudong
    #[serde(default)]
    pub laiyuan: String,
    /// 是否用户手动覆盖（优先于自动价）
    #[serde(default)]
    pub shoudong: bool,
}

/// 行情快照（本地历史，用于趋势图）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kuaizhao {
    /// 快照时间戳（秒）
    pub shijian: i64,
    pub fuwuqi_id: String,
    /// 物品 id -> 当时最低单价（铜）
    pub jiage: BTreeMap<String, Tong>,
}

// ============================================================================
// 计算请求/结果
// ============================================================================

/// 交易行净收入请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetRequest {
    /// 售价（铜/件）
    pub shoujia: Tong,
    /// 挂单数量
    #[serde(default = "one")]
    pub shuliang: u32,
    pub shouxufei: Shouxufei,
    /// 挂单次数（保管费按次计）
    #[serde(default = "one")]
    pub guadan_cishu: u32,
}

fn one() -> u32 {
    1
}

/// 交易行净收入结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetResult {
    /// 成交总额（铜）
    pub zonge: Tong,
    /// 手续费（铜）
    pub feiyong: Tong,
    /// 保管费合计（铜）
    pub baoguanfei: Tong,
    /// 到手（铜）
    pub daoshou: Tong,
}

/// 保本价请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaobenRequest {
    /// 总成本（铜）
    pub chengben: Tong,
    pub shouxufei: Shouxufei,
    #[serde(default = "one")]
    pub guadan_cishu: u32,
}

/// 单配方成本利润请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JisuanRequest {
    pub peifang_id: String,
    /// 制作次数
    #[serde(default = "one")]
    pub cishu: u32,
    /// 每次制作的挂单次数（保管费按次计）
    #[serde(default = "one")]
    pub guadan_cishu: u32,
    /// 每点精力折合金额（铜）
    #[serde(default)]
    pub jingli_danjia: Tong,
    pub shouxufei: Shouxufei,
    /// 物品 id -> 单价（铜）；缺失表示缺价
    #[serde(default)]
    pub jiage: BTreeMap<String, Tong>,
    /// 技艺等级（0-50，暂只影响展示加成，默认 0）
    #[serde(default)]
    pub jiyi_dengji: u32,
}

/// 材料明细行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CailiaoMingxi {
    pub wupin_id: String,
    pub mingcheng: String,
    pub shuliang: u32,
    pub danjia: Tong,
    pub xiaojia: Tong,
    /// hangqing / shangdian / shoudong / quejia
    pub laiyuan: String,
}

/// 产物明细行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChuchanMingxi {
    pub wupin_id: String,
    pub mingcheng: String,
    /// 期望件数
    pub qiwang: f64,
    pub danjia: Tong,
}

/// 单配方成本利润结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JisuanResult {
    pub peifang_id: String,
    pub peifang_mingcheng: String,
    /// 材料成本合计（铜）
    pub chengben_cailiao: Tong,
    /// 精力成本（铜）
    pub chengben_jingli: Tong,
    /// 制作费（铜）
    pub zhizuofei: Tong,
    /// 保管费合计（铜）
    pub baoguanfei: Tong,
    /// 总成本（铜）
    pub zong_chengben: Tong,
    /// 期望产出价值（扣手续费前，铜）
    pub shouru: Tong,
    /// 扣手续费后收入（铜）
    pub jingshouru: Tong,
    /// 净利润（铜）
    pub lirun: Tong,
    /// 利润率（万分比）
    pub lirunlv_bp: i64,
    /// 每精力净利润（铜，可能为负）
    pub mei_jingli: Tong,
    /// 保本单价（铜/件，按期望产出折算）
    pub baobenjia: Tong,
    /// 精力消耗合计
    pub jingli: u32,
    /// 期望产出件数
    pub chuchan_qiwang: f64,
    pub cailiao: Vec<CailiaoMingxi>,
    pub chuchan: Vec<ChuchanMingxi>,
    /// 缺价物品 id 列表（非空表示结果不完整）
    #[serde(default)]
    pub quejia: Vec<String>,
}

/// 排行请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaihangRequest {
    /// 只排某技艺；None = 全部
    #[serde(default)]
    pub jiyi: Option<Jiyi>,
    #[serde(default)]
    pub jingli_danjia: Tong,
    pub shouxufei: Shouxufei,
    #[serde(default)]
    pub jiage: BTreeMap<String, Tong>,
    pub zhibiao: PaihangZhibiao,
    #[serde(default)]
    pub jiyi_dengji: u32,
    /// 最多返回条数；None = 全部
    #[serde(default)]
    pub limit: Option<usize>,
}

/// 排行行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaihangRow {
    pub peifang_id: String,
    pub mingcheng: String,
    pub jiyi: Jiyi,
    pub lirun: Tong,
    pub lirunlv_bp: i64,
    pub mei_jingli: Tong,
    pub jingli: u32,
    /// 是否缺价（缺价的默认不参与排行，但结果里保留供提示）
    pub quejia: bool,
}

/// 五行石转换请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZhuanhuanRequest {
    pub zhuanhuan_id: String,
    #[serde(default = "one")]
    pub cishu: u32,
    #[serde(default)]
    pub jingli_danjia: Tong,
    pub shouxufei: Shouxufei,
    #[serde(default)]
    pub jiage: BTreeMap<String, Tong>,
}

/// 五行石转换结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZhuanhuanResult {
    pub zhuanhuan_id: String,
    pub mingcheng: String,
    /// 期望投入次数（1 / 成功率）
    pub qiwang_cishu: f64,
    /// 期望总成本（铜）
    pub zong_chengben: Tong,
    /// 期望产出价值（扣手续费前）
    pub shouru: Tong,
    /// 净利润（铜）
    pub lirun: Tong,
    pub lirunlv_bp: i64,
    /// 缺价物品 id
    #[serde(default)]
    pub quejia: Vec<String>,
}

// ============================================================================
// 设置 / 刷新进度
// ============================================================================

/// 用户设置（本地存档）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub fuwuqi_id: String,
    pub zhiji_id: String,
    /// 每点精力折合金额（铜）
    #[serde(default)]
    pub jingli_danjia: Tong,
    /// 技艺等级（0-50）
    #[serde(default)]
    pub jiyi_dengji: u32,
    /// 是否联网自动拉价
    #[serde(default = "yes")]
    pub auto_refresh: bool,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fuwuqi_id: "jiandanqinxin".to_string(),
            zhiji_id: "live-130".to_string(),
            jingli_danjia: 0,
            jiyi_dengji: 0,
            auto_refresh: true,
        }
    }
}

/// 行情刷新进度事件负载。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshProgress {
    /// 已完成数量
    pub done: u32,
    /// 总数量
    pub total: u32,
    /// 失败数量
    pub failed: u32,
}

/// 行情刷新结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshResult {
    pub fuwuqi_id: String,
    pub gengxin_shijian: i64,
    pub chenggong: u32,
    pub shibai: u32,
    pub jiage: Vec<JiageJilu>,
}
