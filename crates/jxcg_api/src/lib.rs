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
use std::collections::{BTreeMap, BTreeSet};

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
    /// NPC 商店价（铜）；来自 JX3BOX craft/price，无则缺省。
    /// `> 0` 表示 NPC 无限量售卖：取价时**优先于交易行行情**（手填覆盖仍最高优先）
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

/// 五行石合成方案（游戏内「一键合成/自动合成」使用的配方集合）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WuxingshiFangan {
    pub id: String,
    pub mingcheng: String,
    #[serde(default)]
    pub note: String,
}

/// 五行石合成结果槽（概率结果）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WuxingshiJieguo {
    pub wupin_id: String,
    /// 万分比：9276 = 92.76%；未列出的余量视为失败（材料损失、无产物）
    pub bp: i64,
    /// 命中时的产出数量（默认 1）
    #[serde(default = "one")]
    pub shuliang: u32,
}

/// 五行石单步合成配方（面板口径：主产物 + 材料 + 精力 + 加工费 + 概率结果）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WuxingshiHecheng {
    pub id: String,
    /// 所属合成方案 id
    pub fangan: String,
    /// 主产物物品 id
    pub chanpin: String,
    /// 精力消耗（点/次）
    pub jingli: u32,
    /// 加工费（铜/次）
    pub jiagongfei: Tong,
    #[serde(default)]
    pub cailiao: Vec<Cailiao>,
    #[serde(default)]
    pub jieguo: Vec<WuxingshiJieguo>,
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
    /// 五行石合成方案（data/wuxingshi.toml 的 `[[fangan]]`）
    #[serde(default)]
    pub wuxingshi_fangan: Vec<WuxingshiFangan>,
    /// 五行石单步合成配方（data/wuxingshi.toml 的 `[[hecheng]]`）
    #[serde(default)]
    pub wuxingshi_hecheng: Vec<WuxingshiHecheng>,
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

/// serde 默认值：`true`（新增开关默认开启，保证旧请求获得新口径）。
fn true_default() -> bool {
    true
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
    /// 到手（铜）= 成交总额 − 手续费 − 保管费（可为负，提示不划算）
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
    /// 制作次数（>= 1；批量按期望线性累计）
    #[serde(default = "one")]
    pub cishu: u32,
    /// 每批产出的挂单次数（保管费按次计；0 = 不计保管费）
    #[serde(default = "one")]
    pub guadan_cishu: u32,
    /// 每点精力折合金额（铜）
    #[serde(default)]
    pub jingli_danjia: Tong,
    pub shouxufei: Shouxufei,
    /// 物品 id -> 单价（铜）：该服**全量**行情价 + 手填价（仅供「采购」取价，递归任意深度）。
    /// 取价优先级：手填（`shoudong`）> NPC 商店价（`shangdianjia > 0`）> 本表行情价 > 缺价。
    /// 缺失表示缺价（缺价按 0 计并汇总 `quejia`）。
    #[serde(default)]
    pub jiage: BTreeMap<String, Tong>,
    /// 技艺等级（0-50，暂只影响展示加成，默认 0）
    #[serde(default)]
    pub jiyi_dengji: u32,
    /// 中间产物按自制成本展开（默认 true）；false = 旧行为，按行情价
    #[serde(default = "true_default")]
    pub zhongjian_zizhi: bool,
    /// 手填覆盖的物品 id 集合（**最高取价优先级**，同时让中间产物按手填价采购、不再展开）：
    /// - 取价：id ∈ 本集合且 `jiage` 有值 → 手填价（覆盖 NPC 商店价与行情）；
    /// - 中间产物：命中时不再自制展开；
    /// - 空集 = 全部按自制展开（`zhongjian_zizhi=true` 时）
    #[serde(default)]
    pub shoudong: BTreeSet<String>,
}

/// 材料明细行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CailiaoMingxi {
    pub wupin_id: String,
    pub mingcheng: String,
    pub shuliang: u32,
    pub danjia: Tong,
    pub xiaojia: Tong,
    /// 取价来源：`shoudong`（手填）> `shangdian`（NPC 商店价）> `hangqing`（行情）> `quejia`（缺价）；
    /// 自制中间产物行为 `zizhi`
    pub laiyuan: String,
    /// 该材料行是否为「自制展开」的中间产物（danjia 为单位自制成本）
    #[serde(default)]
    pub zhongjian: bool,
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

/// 中间产物自制成本明细（顶层材料行按自制展开时的一条记录）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZhongjianMingxi {
    pub wupin_id: String,
    pub mingcheng: String,
    /// 采用的产出配方 id（同物品多配方取 (dengji, id) 最小者）
    pub peifang_id: String,
    /// 顶层需求数量（材料需求量 × 制作次数）
    pub shuliang: u32,
    /// 单位自制成本（含材料 + 精力 + 制作费，HALF_UP 到铜）
    pub danjia: Tong,
    /// 摊入小计 = danjia × shuliang
    pub xiaojia: Tong,
    /// 该中间产物链路消耗的精力合计（按顶层需求数量折算）
    pub jingli: u32,
    /// 其一层采购原料（供 UI 展开；可含更深中间产物行，数量按顶层需求折算）
    pub cailiao: Vec<CailiaoMingxi>,
}

/// 单配方成本利润结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JisuanResult {
    pub peifang_id: String,
    pub peifang_mingcheng: String,
    /// 材料成本合计（铜）= Σ 顶层材料行小计；自制中间产物行按「单位自制成本 × 数量」计入
    pub chengben_cailiao: Tong,
    /// 最终配方自身精力成本（铜，不含中间产物链路精力）
    pub chengben_jingli: Tong,
    /// 最终配方自身制作费（铜，不含中间产物）
    pub zhizuofei: Tong,
    /// 保管费合计（铜）
    pub baoguanfei: Tong,
    /// 材料成本中「自制中间产物」部分（展示用；已含在 chengben_cailiao，不重复加总）
    #[serde(default)]
    pub chengben_zhongjian: Tong,
    /// 中间产物链路消耗的精力合计（不含最终配方自身精力）
    #[serde(default)]
    pub jingli_zhongjian: u32,
    /// 总成本（铜）= chengben_cailiao + chengben_jingli + zhizuofei + baoguanfei
    pub zong_chengben: Tong,
    /// 单件成本（铜/件）= zong_chengben / 期望产出总件数（HALF_UP）；
    /// 期望产出为 0 时等于 zong_chengben。
    /// 多产出配方（如每次期望 2.5 件）用它区分「每批总成本」与「单件成本」。
    #[serde(default)]
    pub danjia_chengben: Tong,
    /// 期望产出价值（扣手续费前，铜）
    pub shouru: Tong,
    /// 扣手续费后收入（铜）
    pub jingshouru: Tong,
    /// 净利润（铜）
    pub lirun: Tong,
    /// 利润率（万分比）
    pub lirunlv_bp: i64,
    /// 每精力净利润（铜，可能为负）= 净利润 / (最终精力 + 中间产物精力)
    pub mei_jingli: Tong,
    /// 保本单价（铜/件，按期望产出折算）
    pub baobenjia: Tong,
    /// 最终配方自身精力消耗合计（不含中间产物；中间产物精力见 `jingli_zhongjian`）
    pub jingli: u32,
    /// 期望产出件数
    pub chuchan_qiwang: f64,
    pub cailiao: Vec<CailiaoMingxi>,
    /// 自制展开的中间产物明细
    #[serde(default)]
    pub zhongjian: Vec<ZhongjianMingxi>,
    pub chuchan: Vec<ChuchanMingxi>,
    /// 缺价物品 id 列表（非空表示结果不完整）
    #[serde(default)]
    pub quejia: Vec<String>,
    /// 「无行情/手填价、也无 NPC 商店价、且没有任何产出配方」的材料 id（`quejia` 的子集）。
    /// 这类物品无法靠行情/商店/自制获得（可能为采集材料或旧物品），需人工补价或清理数据。
    #[serde(default)]
    pub bukede: Vec<String>,
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
    /// 物品 id -> 单价（铜）：该服**全量**行情价 + 手填价（采购取价；中间产物默认自制展开）。
    /// 取价优先级：手填（`shoudong`）> NPC 商店价 > 本表行情价 > 缺价
    #[serde(default)]
    pub jiage: BTreeMap<String, Tong>,
    pub zhibiao: PaihangZhibiao,
    #[serde(default)]
    pub jiyi_dengji: u32,
    /// 最多返回条数；None = 全部
    #[serde(default)]
    pub limit: Option<usize>,
    /// 是否包含缺价配方（默认 false = 排除；true 时缺价行排在末尾并置 `quejia = true`）
    #[serde(default)]
    pub baohan_quejia: bool,
    /// 中间产物按自制成本展开（默认 true）
    #[serde(default = "true_default")]
    pub zhongjian_zizhi: bool,
    /// 是否包含「只产中间产物」的配方（默认 false = 不参与排行）
    #[serde(default)]
    pub baohan_zhongjian: bool,
    /// 手填覆盖的物品 id 集合（最高取价优先级；语义同 `JisuanRequest.shoudong`）
    #[serde(default)]
    pub shoudong: BTreeSet<String>,
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
    /// 该配方是否只产中间产物（baohan_zhongjian=true 时返回）
    #[serde(default)]
    pub zhongjian: bool,
}

/// 五行石转换请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZhuanhuanRequest {
    pub zhuanhuan_id: String,
    /// 目标成功次数（产出高阶石个数，>= 1）；期望投入次数 = cishu × 1/p
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
    /// 期望投入总次数（cishu × 1/p，p = chenggonglv_bp / 10000）
    pub qiwang_cishu: f64,
    /// 期望总成本（铜）= 单次投入成本 × cishu / p
    pub zong_chengben: Tong,
    /// 期望产出价值（扣手续费前，铜）= cishu × 高阶石单价
    pub shouru: Tong,
    /// 净利润（铜）
    pub lirun: Tong,
    pub lirunlv_bp: i64,
    /// 缺价物品 id
    #[serde(default)]
    pub quejia: Vec<String>,
}

/// 五行石一键合成请求（面板配方链，目标默认六级 `5_24428`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WuxingshiLianRequest {
    pub fangan_id: String,
    /// 目标物品 id（六级 = `5_24428`）
    pub mubiao_wupin_id: String,
    /// 目标件数
    #[serde(default = "one")]
    pub shuliang: u32,
    /// 每点精力折合金额（铜）
    #[serde(default)]
    pub jingli_danjia: Tong,
    pub shouxufei: Shouxufei,
    /// 物品 id -> 单价（铜）：采购取价优先级 **NPC 商店价（> 0）> 本表行情价 > 缺价**
    /// （本请求没有手填覆盖字段；缺价按 0 计并进 `quejia`）
    #[serde(default)]
    pub jiage: BTreeMap<String, Tong>,
    /// 覆盖「合 6 意外出 8 级」概率（万分比）；None = 用数据里的 bp（0 = 忽略）
    #[serde(default)]
    pub baoji_bp: Option<i64>,
}

/// 五行石合成副产物（期望量）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WuxingshiFuchanpin {
    pub wupin_id: String,
    pub mingcheng: String,
    /// 期望产出件数
    pub qiwang: f64,
    pub danjia: Tong,
    /// 折价收入（铜，HALF_UP）；缺价时为 0
    pub shouru: Tong,
}

/// 五行石合成步骤（按单步配方聚合的期望值）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WuxingshiBuzhou {
    pub hecheng_id: String,
    pub chanpin: String,
    pub mingcheng: String,
    /// 期望合成次数（含成功率放大）
    pub cishu: f64,
    /// 该步骤精力合计（期望，HALF_UP 到整数）
    pub jingli: u32,
    /// 该步骤加工费合计（期望，HALF_UP 到铜）
    pub jiagongfei: Tong,
}

/// 五行石一键合成结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WuxingshiLianResult {
    pub fangan_id: String,
    pub fangan_mingcheng: String,
    pub mubiao_wupin_id: String,
    pub mubiao_mingcheng: String,
    pub shuliang: u32,
    /// 采购原料明细（一级石等无配方原料；数量为期望值 HALF_UP 到整数，展示口径）
    pub cailiao: Vec<CailiaoMingxi>,
    /// 材料成本（铜，期望值一次性 HALF_UP）
    pub cailiao_chengben: Tong,
    /// 精力合计（点，期望值 HALF_UP 到整数）
    pub jingli: u32,
    /// 精力折金（铜）
    pub jingli_chengben: Tong,
    /// 加工费合计（铜）
    pub jiagongfei: Tong,
    /// 副产物期望收益明细
    pub fuchanpin: Vec<WuxingshiFuchanpin>,
    /// 副产物折价收入合计（铜，冲抵成本）
    pub fuchanpin_shouru: Tong,
    /// 净成本（铜）= 材料 + 精力折金 + 加工费 − 副产物收入
    pub zong_chengben: Tong,
    /// 单位净成本（铜/颗，= zong_chengben / shuliang 四舍五入）
    pub danjia: Tong,
    /// 保本单价（铜/颗，按 5% 手续费反推的最小挂单价）
    pub baobenjia: Tong,
    /// 目标物市场价（铜/颗；缺价为 0 并进 quejia）
    pub shichangjia: Tong,
    /// 净利润（铜）= 市场价 × 数量 − 手续费 − 净成本
    pub lirun: Tong,
    /// 利润率（万分比）
    pub lirunlv_bp: i64,
    /// 合成步骤明细（按数据文件顺序，仅含实际用到的配方）
    pub buzhou: Vec<WuxingshiBuzhou>,
    /// 缺价物品 id（排序去重）
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
    /// 中间产物按自制成本展开（默认 true）
    #[serde(default = "true_default")]
    pub zhongjian_zizhi: bool,
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
            zhongjian_zizhi: true,
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

// ============================================================================
// DTO 契约测试（冻结前后端/命令共享的序列化形状与默认值）
//
// 这些测试是「向后兼容」的红线：
// - 旧前端不传可选字段时必须能反序列化（serde default）；
// - 枚举的 snake_case 序列化名是 Tauri 命令与前端类型的共同协议；
// - Catalog 全字段 JSON 往返必须一致。
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_catalog() -> Catalog {
        Catalog {
            banben: Banben {
                level: 130,
                season: 3,
                modified: 20261112,
                note: "测试".into(),
            },
            wupin: vec![Wupin {
                id: "5_1".into(),
                mingcheng: "材料".into(),
                pinlei: Pinlei::Cailiao,
                dui: Some(20),
                shangdianjia: Some(100),
                laiyuan: Some("jx3box".into()),
            }],
            peifang: vec![Peifang {
                id: "cooking_1".into(),
                mingcheng: "测试配方".into(),
                jiyi: Jiyi::Pengren,
                dengji: 10,
                jingli: 5,
                zhizuofei: 100,
                cailiao: vec![Cailiao {
                    wupin_id: "5_1".into(),
                    shuliang: 2,
                }],
                chuchan: vec![Chuchan {
                    wupin_id: "5_2".into(),
                    min: 1,
                    max: 2,
                    quanzhong: QUANZHONG_FENMU,
                }],
                laiyuan: Some("jx3box".into()),
            }],
            wuxingshi: vec![Zhuanhuan {
                id: "w1".into(),
                mingcheng: "转换".into(),
                cong: "5_1".into(),
                dao: "5_2".into(),
                xuyao: 3,
                jingli: 10,
                jinbi: 100,
                chenggonglv_bp: 4_878,
                laiyuan: Some("shequ".into()),
            }],
            wuxingshi_fangan: vec![WuxingshiFangan {
                id: "mianban".into(),
                mingcheng: "面板配方".into(),
                note: "测试".into(),
            }],
            wuxingshi_hecheng: vec![WuxingshiHecheng {
                id: "mianban_3".into(),
                fangan: "mianban".into(),
                chanpin: "5_2".into(),
                jingli: 10,
                jiagongfei: 3_600,
                cailiao: vec![Cailiao {
                    wupin_id: "5_1".into(),
                    shuliang: 12,
                }],
                jieguo: vec![
                    WuxingshiJieguo {
                        wupin_id: "5_2".into(),
                        bp: 9_276,
                        shuliang: 1,
                    },
                    WuxingshiJieguo {
                        wupin_id: "5_3".into(),
                        bp: 724,
                        shuliang: 1,
                    },
                ],
            }],
            fuwuqi: vec![Fuwuqi {
                id: "jiandanqinxin".into(),
                mingcheng: "剑胆琴心".into(),
                daju: Some("电信区".into()),
            }],
            zhiji: vec![Zhiji {
                id: "live-130".into(),
                mingcheng: "正式服".into(),
                note: "备注".into(),
                shouxufei: Shouxufei::default(),
            }],
        }
    }

    #[test]
    fn money_unit_contract() {
        // 金额一律 i64 铜（8 字节），权重分母固定 2^20
        assert_eq!(std::mem::size_of::<Tong>(), 8);
        let _: Tong = i64::MAX;
        assert_eq!(QUANZHONG_FENMU, 1_048_576);
        assert_eq!(QUANZHONG_FENMU, 1 << 20);
        // 常见换算：1 金 = 100 银 = 10_000 铜；1 砖 = 10_000 金 = 100_000_000 铜
        assert_eq!(10_000 / 100, 100);
        assert_eq!(100_000_000 / 10_000, 10_000);
    }

    #[test]
    fn shouxufei_default() {
        let cfg = Shouxufei::default();
        assert_eq!(cfg.shuilv_bp, 500); // 5%
        assert_eq!(cfg.quzheng, Quzheng::HalfUp);
        assert_eq!(cfg.baoguanfei, 8_000); // 80 银
    }

    #[test]
    fn enum_serde_names() {
        // 枚举的 snake_case 名是命令参数协议，不能随意变更
        assert_eq!(
            serde_json::to_string(&Jiyi::Pengren).unwrap(),
            "\"pengren\""
        );
        assert_eq!(serde_json::to_string(&Jiyi::Zhiyao).unwrap(), "\"zhiyao\"");
        assert_eq!(
            serde_json::from_str::<Jiyi>("\"zijiang\"").unwrap(),
            Jiyi::Zijiang
        );
        assert_eq!(
            serde_json::to_string(&Pinlei::Wuxingshi).unwrap(),
            "\"wuxingshi\""
        );
        assert_eq!(
            serde_json::to_string(&Quzheng::HalfUp).unwrap(),
            "\"half_up\""
        );
        assert_eq!(serde_json::to_string(&Quzheng::Floor).unwrap(), "\"floor\"");
        assert_eq!(
            serde_json::to_string(&PaihangZhibiao::MeiJingli).unwrap(),
            "\"mei_jingli\""
        );
        assert_eq!(
            serde_json::from_str::<PaihangZhibiao>("\"lirunlv\"").unwrap(),
            PaihangZhibiao::Lirunlv
        );
    }

    #[test]
    fn jiyi_helpers() {
        assert_eq!(Jiyi::ALL.len(), 5);
        for jiyi in Jiyi::ALL {
            assert_eq!(Jiyi::from_id(jiyi.id()), Some(jiyi));
            assert!(!jiyi.mingcheng().is_empty());
        }
        assert_eq!(Jiyi::from_id("unknown"), None);
    }

    #[test]
    fn request_defaults_are_backward_compatible() {
        let shouxufei = r#"{"shuilv_bp":500,"quzheng":"half_up","baoguanfei":8000}"#;

        // NetRequest：缺省 shuliang/guadan_cishu 默认 1
        let net: NetRequest =
            serde_json::from_str(&format!(r#"{{"shoujia":100,"shouxufei":{shouxufei}}}"#)).unwrap();
        assert_eq!(net.shuliang, 1);
        assert_eq!(net.guadan_cishu, 1);

        // JisuanRequest：缺省批量/精力单价/价格表/技艺等级
        let jisuan: JisuanRequest =
            serde_json::from_str(&format!(r#"{{"peifang_id":"p1","shouxufei":{shouxufei}}}"#))
                .unwrap();
        assert_eq!(jisuan.cishu, 1);
        assert_eq!(jisuan.guadan_cishu, 1);
        assert_eq!(jisuan.jingli_danjia, 0);
        assert!(jisuan.jiage.is_empty());
        assert_eq!(jisuan.jiyi_dengji, 0);
        assert!(jisuan.zhongjian_zizhi, "新增开关默认 true");
        assert!(jisuan.shoudong.is_empty(), "手填集合默认空");

        // PaihangRequest：缺省 jiyi/limit/jiyi_dengji/jiage，且新增 baohan_quejia 默认 false
        let paihang: PaihangRequest =
            serde_json::from_str(&format!(r#"{{"shouxufei":{shouxufei},"zhibiao":"lirun"}}"#))
                .unwrap();
        assert_eq!(paihang.jiyi, None);
        assert_eq!(paihang.limit, None);
        assert_eq!(paihang.jiyi_dengji, 0);
        assert!(paihang.jiage.is_empty());
        assert!(!paihang.baohan_quejia, "新增字段必须默认 false（缺价排除）");
        assert!(paihang.zhongjian_zizhi, "新增开关默认 true");
        assert!(
            !paihang.baohan_zhongjian,
            "新增开关默认 false（中间产物配方排除）"
        );
        assert!(paihang.shoudong.is_empty());

        // ZhuanhuanRequest：缺省 cishu/精力单价/价格表
        let zhuanhuan: ZhuanhuanRequest = serde_json::from_str(&format!(
            r#"{{"zhuanhuan_id":"w1","shouxufei":{shouxufei}}}"#
        ))
        .unwrap();
        assert_eq!(zhuanhuan.cishu, 1);
        assert_eq!(zhuanhuan.jingli_danjia, 0);
        assert!(zhuanhuan.jiage.is_empty());

        // BaobenRequest
        let baoben: BaobenRequest =
            serde_json::from_str(&format!(r#"{{"chengben":100,"shouxufei":{shouxufei}}}"#))
                .unwrap();
        assert_eq!(baoben.guadan_cishu, 1);
    }

    #[test]
    fn optional_fields_default() {
        let wupin: Wupin =
            serde_json::from_str(r#"{"id":"x","mingcheng":"y","pinlei":"cailiao"}"#).unwrap();
        assert_eq!(wupin.dui, None);
        assert_eq!(wupin.shangdianjia, None);
        assert_eq!(wupin.laiyuan, None);

        let peifang: Peifang = serde_json::from_str(
            r#"{"id":"p","mingcheng":"n","jiyi":"pengren","dengji":1,"jingli":0}"#,
        )
        .unwrap();
        assert_eq!(peifang.zhizuofei, 0);
        assert!(peifang.cailiao.is_empty());
        assert!(peifang.chuchan.is_empty());
        assert_eq!(peifang.laiyuan, None);

        let chuchan: Chuchan = serde_json::from_str(r#"{"wupin_id":"x","min":1,"max":2}"#).unwrap();
        assert_eq!(chuchan.quanzhong, 0);

        // 材料明细新增的 zhongjian 标记缺省为 false（旧数据/旧前端兼容）
        let cailiao: CailiaoMingxi = serde_json::from_str(
            r#"{"wupin_id":"x","mingcheng":"y","shuliang":1,"danjia":2,"xiaojia":2,"laiyuan":"hangqing"}"#,
        )
        .unwrap();
        assert!(!cailiao.zhongjian);

        let zhuanhuan: Zhuanhuan = serde_json::from_str(
            r#"{"id":"w","mingcheng":"n","cong":"a","dao":"b","xuyao":1,"jingli":1,"chenggonglv_bp":5000}"#,
        )
        .unwrap();
        assert_eq!(zhuanhuan.jinbi, 0);
        assert_eq!(zhuanhuan.laiyuan, None);

        let jiage: JiageJilu = serde_json::from_str(r#"{"wupin_id":"x","fuwuqi_id":"s"}"#).unwrap();
        assert!(jiage.guadan.is_empty());
        assert_eq!(jiage.gengxin_shijian, 0);
        assert_eq!(jiage.laiyuan, "");
        assert!(!jiage.shoudong);
    }

    #[test]
    fn catalog_roundtrip() {
        let catalog = sample_catalog();
        let json = serde_json::to_string(&catalog).unwrap();
        let back: Catalog = serde_json::from_str(&json).unwrap();
        assert_eq!(back, catalog);
        // 嵌套结构确实被序列化（不是依赖 serde skip）
        assert!(json.contains("\"peifang\""));
        assert!(json.contains("\"shouxufei\""));
        assert!(json.contains("\"quanzhong\""));
    }

    #[test]
    fn result_roundtrip() {
        let result = JisuanResult {
            peifang_id: "p".into(),
            peifang_mingcheng: "配方".into(),
            chengben_cailiao: 1,
            chengben_jingli: 2,
            zhizuofei: 3,
            baoguanfei: 4,
            chengben_zhongjian: 6,
            jingli_zhongjian: 2,
            zong_chengben: 10,
            danjia_chengben: 10,
            shouru: 20,
            jingshouru: 19,
            lirun: 9,
            lirunlv_bp: 9_000,
            mei_jingli: 3,
            baobenjia: 11,
            jingli: 5,
            chuchan_qiwang: 1.5,
            cailiao: vec![],
            zhongjian: vec![ZhongjianMingxi {
                wupin_id: "zj".into(),
                mingcheng: "中间产物".into(),
                peifang_id: "p_zj".into(),
                shuliang: 2,
                danjia: 3,
                xiaojia: 6,
                jingli: 2,
                cailiao: vec![CailiaoMingxi {
                    wupin_id: "5_1".into(),
                    mingcheng: "原料".into(),
                    shuliang: 1,
                    danjia: 2,
                    xiaojia: 2,
                    laiyuan: "hangqing".into(),
                    zhongjian: false,
                }],
            }],
            chuchan: vec![ChuchanMingxi {
                wupin_id: "x".into(),
                mingcheng: "产物".into(),
                qiwang: 1.5,
                danjia: 7,
            }],
            quejia: vec!["x".into()],
            bukede: vec!["x".into()],
        };
        let back: JisuanResult =
            serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
        assert_eq!(back, result);

        let zhuanhuan = ZhuanhuanResult {
            zhuanhuan_id: "w".into(),
            mingcheng: "转换".into(),
            qiwang_cishu: 2.0,
            zong_chengben: 100,
            shouru: 200,
            lirun: 90,
            lirunlv_bp: 9_000,
            quejia: vec![],
        };
        let back: ZhuanhuanResult =
            serde_json::from_str(&serde_json::to_string(&zhuanhuan).unwrap()).unwrap();
        assert_eq!(back, zhuanhuan);
    }

    #[test]
    fn wuxingshi_contract() {
        let shouxufei = r#"{"shuilv_bp":500,"quzheng":"half_up","baoguanfei":8000}"#;
        // 请求缺省：shuliang=1、精力单价 0、价格表空、baoji_bp=None
        let req: WuxingshiLianRequest = serde_json::from_str(&format!(
            r#"{{"fangan_id":"mianban","mubiao_wupin_id":"5_24428","shouxufei":{shouxufei}}}"#
        ))
        .unwrap();
        assert_eq!(req.shuliang, 1);
        assert_eq!(req.jingli_danjia, 0);
        assert!(req.jiage.is_empty());
        assert_eq!(req.baoji_bp, None);

        // 结果槽 shuliang 缺省为 1
        let jieguo: WuxingshiJieguo =
            serde_json::from_str(r#"{"wupin_id":"5_24430","bp":5}"#).unwrap();
        assert_eq!(jieguo.shuliang, 1);
        // hecheng 的 cailiao/jieguo 缺省为空
        let hecheng: WuxingshiHecheng = serde_json::from_str(
            r#"{"id":"h1","fangan":"mianban","chanpin":"5_24425","jingli":10,"jiagongfei":3600}"#,
        )
        .unwrap();
        assert!(hecheng.cailiao.is_empty());
        assert!(hecheng.jieguo.is_empty());

        // Catalog JSON 往返包含 new sections
        let catalog = sample_catalog();
        let json = serde_json::to_string(&catalog).unwrap();
        assert!(json.contains("wuxingshi_hecheng"));
        assert!(json.contains("wuxingshi_fangan"));
        let back: Catalog = serde_json::from_str(&json).unwrap();
        assert_eq!(back, catalog);
        // 旧 Catalog JSON（无新段）也能反序列化
        let old_json = r#"{"banben":{"level":1,"season":1,"modified":1},"wupin":[],"peifang":[],"wuxingshi":[],"fuwuqi":[],"zhiji":[]}"#;
        let old: Catalog = serde_json::from_str(old_json).unwrap();
        assert!(old.wuxingshi_fangan.is_empty());
        assert!(old.wuxingshi_hecheng.is_empty());
    }

    #[test]
    fn settings_default() {
        let s = Settings::default();
        assert_eq!(s.fuwuqi_id, "jiandanqinxin");
        assert_eq!(s.zhiji_id, "live-130");
        assert_eq!(s.jingli_danjia, 0);
        assert_eq!(s.jiyi_dengji, 0);
        assert!(s.auto_refresh);
        assert!(s.zhongjian_zizhi, "中间产物自制开关默认 true");
        // 缺字段时 auto_refresh 默认 true
        let s: Settings = serde_json::from_str(r#"{"fuwuqi_id":"a","zhiji_id":"b"}"#).unwrap();
        assert!(s.auto_refresh);
        assert!(s.zhongjian_zizhi);
    }
}
