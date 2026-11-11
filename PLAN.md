# JXCG — 剑网3 交易行计算器（方案 v1）

> 状态：**决策基本拍板，第三方行情 API 已实测**。细节待你确认的只剩「行情接入方式」与「手续费口径」。
> 定位：与 `../jpcg` 同风格的桌面工具——把「交易行行情 → 制作成本 → 税后收益 → 值不值得做」算清楚。

## 0. 决策记录（2026-11-12）

| # | 决策 | 结果 |
|---|------|------|
| D1 | 首版范围 | ✅ **MVP：计算（净收入/保本价）+ 单配方利润 + 行情库 + 全配方排行** |
| D2 | 行情来源 | ✅ **接 JX3BOX 自动行情 + 手动覆盖；断网/失败回落最后一次拉取的缓存价（last-known-good），无缓存再用手动价**（见 §7.1） |
| D3 | 首批数据 | ✅ **全部生活技艺（烹饪/制药/缝纫/铸造/梓匠）+ 五行石精炼** |
| D4 | 工程规格 | ✅ **同技术栈裁剪版：Tauri v2 + Rust core + React 19**（不做 updater/FFI/论坛/动态 dll） |
| D5 | 费用口径 | ✅ **5% 四舍五入（HALF_UP）+ 保管费 80 银/24h**，全部做成值集可改 |
| D6 | 名称/许可 | ✅ **jxcg / GPL-3.0** |
| D7 | 包管理器 | ✅ **bun**（用户指定，替代 npm）：`bun install` / `bun run build` / `bun run test`，锁文件 `bun.lock` |

## 1. 背景与目标

- **同风格承担什么**：沿用 JPCG 的桌面形态（Tauri v2 + Rust core + React 19 + Vite）、
  薄荷/松石绿设计 token、左侧图标导航 + 页面保活 + 状态栏 + Toast、`data/` TOML + 变更日志 + Makefile/CI 的工程纪律。
- **不照搬什么**：JPCG 的 FFI/动态 dll、模块更新、论坛、连招/配装搜索等重资产 v0 不需要；
  本工具计算轻、数据为「物品/配方/行情」，引擎以纯函数为主。
- **目标用户**：生活技艺玩家（制药/烹饪/缝纫/铸造/梓匠）、搬砖党、交易行倒卖、五行石精炼党。
- **要回答的问题**：
  1. 挂这个价，**到手多少**？最少挂多少不亏（保本价）？
  2. 做这个东西卖，**赚不赚**？扣掉材料、精力、手续费、保管费之后净利润多少？
  3. 现在**做什么最赚**？（全配方按 净利润/精力 排行）
  4. 这个材料/成品**最近涨了还是跌了**？（行情快照与趋势）

## 2. 产品闭环

```
录行情（JX3BOX 自动 + 手动覆盖/粘贴/导入） → 绑定服务器 → 选配方/物品
   → 材料清单带出单价（行情 > 商店价 > 手动） → 成本（材料 + 精力 + 制作费）
   → 售价 × (1 - 5%) - 保管费 → 净利润 / 利润率 / 每精力收益
   → 保本价 & 全配方排行 & 快照趋势 → 决策
```

## 3. 功能范围（v0 先做 ✅）

| 模块 | 内容 | v0 |
|------|------|----|
| 交易行净收入 | 金/银/铜输入输出、5% 手续费（HALF_UP）、保管费、到手价、保本价、批量 | ✅ |
| 单配方成本利润 | 材料清单（8 槽）、行情/商店/手填价格、精力成本、制作费、产物期望（概率权重）、净利润/利润率/每精力收益 | ✅ |
| 五行石精炼 | 转换配方模型：低阶石 × n + 精力 + 金币 + 成功率 → 高阶石；按期望成本/利润计算 | ✅ |
| 行情库 | JX3BOX 自动拉价、手动覆盖、搜索/收藏、批量粘贴导入、JSON 导出导入、服务器切换 | ✅ |
| 快照与趋势 | 每次刷新自动存快照、单物品价格趋势图（recharts） | ✅ |
| 全配方排行 | 按 净利润 / 净利润/精力 / 利润率 排序，缺价过滤，TOP 图表 | ✅ |
| 手动模式 | 不绑配方，直接输成本构成 + 售价 | ✅ |
| 倒卖/囤货 | 买入价、目标卖出价、持有期、ROI | 远期 |
| 万宝楼 | 金币/外观/角色 4%~5% 手续费模型 | 不做（与交易行分开） |

## 4. 交互与页面（JPCG 同款壳）

- 布局固定：`ActivityBar`（左侧图标栏，可展开）→ header（渐变 logo + 反馈 + 主题切换）→ 内容区 → `StatusBar` → `Toast`；
  页面用 `visitedPages` 保活（切页不丢状态），深/浅色主题跟随 jpcg 的 `variables.css` token。
- **计算页**：顶部模式栏「配方模式 / 手动模式」；
  左：服务器 + 配方搜索（技艺筛选/收藏）+ 制作次数 + 精力单价 + 技艺等级（0–50，影响产出加成）；
  中：材料表（名称 / 需求数 / 单价 / 小计 / 来源标记：行情·商店·手填，单价可改）；
  右：结果卡（总成本、到手、净利润、利润率、每精力利润、保本价、期望产出）+ 明细。
- **行情页**：物品价格表（搜索、类别筛选、收藏、更新时间/来源）、一键刷新（JX3BOX，带进度与失败降级）、
  手动覆盖/新增、批量粘贴导入、保存快照、历史快照、单物品趋势图、导出/导入 JSON。
- **排行页**：指标切换 + 技艺筛选 + 排行表 + TOP N 柱状图；点行跳转计算页；顶部显示行情完整度（多少材料有价）。
- **设置页**：服务器、值集（费率 5% / 取整 HALF_UP / 保管费 80 银）、精力折算、技艺等级、主题、数据版本、存档导入导出、关于。
- 通用控件：`MoneyInput`（金/银/铜三段或文本解析 `1金20银3铜`，支持 `砖`：1 砖 = 10000 金）、`ResultCard`、`Toast`、`StatusBar`。

## 5. 技术架构

### 5.1 Rust workspace（对齐 jpcg 的目录习惯，按需裁剪）

```
jxcg/
├── Cargo.toml                     # workspace，edition 2024
├── crates/
│   ├── jxcg_api/                  # 纯 DTO 契约（serde），前后端单一来源
│   ├── jxcg_core/                 # 纯计算：金额/费率/成本/期望产出/排行 + 数据装载（无网络）
│   └── jxcg_market/               # 行情 provider：JX3BOX 客户端 + 缓存/节流/降级（reqwest）
├── examples/jxcg_app/
│   ├── src/                       # React 19 + TS + Vite + CSS Modules（复刻 jpcg 壳）
│   └── src-tauri/                 # Tauri v2，commands 按域拆分：calc/market/data/settings
├── server_tools/jx3box-export/    # 构建期数据导出：配方/物品/商店价/服务器 → data/*.toml
├── data/                          # 随包数据（TOML，见 §7）
├── changes/                       # 每次改动一条 YYYY-MM-DD-HHMMSS.md（同 jpcg 约定）
├── Makefile / scripts/ / .github/workflows/
└── AGENTS.md / README.md / CONTRIBUTING.md / PLAN.md
```

- **v0 不做**：FFI、动态 dll、`jxcg_update`/updater、论坛、数据通道。计算是纯 Rust 函数，Tauri 直接静态调用 `jxcg_core`。
- `jxcg_core` **不联网**（金标准测试不受网络影响）；`jxcg_market` 负责所有 HTTP，可被 mock。
- **错误处理**：库代码不用 `unwrap/expect`，统一 `Result<_, Error>`；Tauri command 返回中文错误串（同 jpcg 教训）。
- **金额全程整数**：内部以「铜」为单位的 `i64`，禁止浮点；费率用万分比 `i64`（500 = 5%）；概率权重用 `u32`（分母 2^20）。

### 5.2 前端复用清单（从 jpcg 移植/改写，保持同一视觉语言）

`src/styles/variables.css` + `reset.css`（主题 token）、`App.module.css`（布局）、`ActivityBar`、`StatusBar`、`Toast`、
`ThemeToggle`、`hooks/useTheme`、`hooks/usePersistentState`、`hooks/useToast`、`components/icons.tsx`（新增钱袋/榜单/行情图标）、
`api/commands.ts`（`window.__TAURI__` + 浏览器 mock 兜底）、`utils/clsx.ts`；`utils/format.ts` 扩展金/银/铜/砖格式化。依赖管理用 **bun**（`bun.lock`）。

### 5.3 Tauri 命令（按域）

- calc：`calculate_net`（净收入/保本）、`calculate_recipe`（单配方）、`rank_recipes`（排行）、`calculate_convert`（五行石转换）
- market：`load_prices`、`upsert_price`、`delete_price`、`import_prices`、`export_prices`、`refresh_prices`（JX3BOX 拉取，进度事件）、`save_snapshot`、`list_snapshots`、`load_snapshot`
- data：`load_catalog`（物品 + 配方 + 服务器 + 值集 + 商店价）、`reload_data`
- settings：`load_settings`、`save_settings`、`load_value_sets`、`select_value_set`

## 6. 计算模型（先把口径定死，测试才好写）

### 6.1 单位与解析
- `1 金 = 100 银 = 10000 铜`；`1 砖 = 10000 金`（社区口径，用于大额展示）。显示：`1金20银3铜`（0 项省略）。
- 输入解析兼容：`123金45银67铜`、`123金`、`123.45`（金，两位小数）、纯数字默认按「金」；解析失败给中文错误提示。
- 所有金额、成本、利润在 core 内以 `i64` 铜运算；排名/展示处再格式化。

### 6.2 手续费、保管费与净收入
- 交易行卖方手续费 **5%**，按成交总价计算，**四舍五入到铜（HALF_UP）**；另有 **保管费 80 银/24h（8000 铜）**。
  > 口径依据：社区开源实现 [wind-jx3](https://github.com/hidewnd-winds/wind-jx3)（Apache-2.0）的 `CostCalculator`
  > （feeRate=0.05、HALF_UP、custodyFee=8000），与 hidewnd 成本接口的利润数一致（`利润 = 交易行总价 − 5% − 8000 − 成本`）。
  > 上线前用游戏内一单实测校准（D5）。
- `净收入 = 售价 − 手续费`；`净利润 = 净收入 − 保管费 − 总成本`。
- `保本价 = 满足 净收入 − 保管费 ≥ 成本 的最小售价`（5% 时约 `(成本+保管费)/0.95`，逐铜试探避免差 1 铜）。
- 费率、取整方式、保管费放入「值集」（`data/values/`），界面可切、可改；默认锁 5% / HALF_UP / 8000 铜。
- 挂单时长先按单档 24h；字段预留 `guaha_shichang`（小时）与按档位扩展。

### 6.3 制作利润
- `材料成本 = Σ(需求数量 × 单价)`；单价默认取行情库，可逐项手改（标记「行情/商店/手填/自采」）。
- `精力成本 = 精力消耗 × 精力单价`（精力单价为用户设置的可调参数，默认 0 并在结果里单独列出）。
- `制作费 = 配方固定金币消耗`（铜）。
- **产物期望**：配方数据自带 8 个产出槽，每槽有权重 `w_i`（分母 2^20 = 1048576，普通品 96%/精制品 4% 即权重 1006633/41943）、
  数量区间 `[min,max]`；`每次期望产出 = Σ (w_i/2^20) × (min_i+max_i)/2`；批量 N 次按期望累计，展示时向下取整（社区口径）。
- **技艺等级加成**（0–50）：按社区口径 30/40 级各增加幸运概率、35/45 级各增加特殊产物概率（各 31/1024），先作为可开关的高级选项，默认 0 加成。
- **五行石精炼**单列模型：`n 个低阶石 + 精力 + 金币`，成功率 `p` → 期望成本按 `1/p` 次投入（几何分布期望），成功率/规则整理成 `data/wuxingshi.toml`。
- `净利润 = Σ(产出 × 售价 × (1−费率)) − 材料成本 − 精力成本 − 制作费 − 保管费×挂单次数`；
  `利润率 = 净利润 / 总成本`；`每精力利润 = 净利润 / 精力消耗`。
- 排行指标三选一：净利润/次、净利润/精力、利润率；材料缺价的配方标「缺价」并默认排除。

### 6.4 示例（口径演示，非真实价格）
> 材料 800 金、精力 100 点（1 点 = 2 金 → 200 金）、制作费 0 → 总成本 1000 金；
> 售价 1200 金，手续费 5% = 60 金（HALF_UP），保管费 80 银 = 0.8 金，净收入 1140 金；
> 净利润 139.2 金 ≈ 139 金 20 银，利润率 13.9%，每精力利润 1.39 金。

## 7. 数据模型（TOML，键名沿用 jpcg 的拼音/中文注释风格）

```toml
# data/wupin.toml —— 物品
[version]
level = 130
season = 3
modified = 20261112

[[wupin]]
id = "5_30824"
mingcheng = "精肉"
pinlei = "cailiao"        # cailiao 材料 / chengpin 成品 / xiaohao 消耗品 / wuxingshi 五行石
dui = 20                  # 一组堆叠数（按组购买用）
shangdianjia = 1000       # 商店价（铜，来自 craft/price；无则省略）
laiyuan = "jx3box"        # 数据来源标识

# data/peifang.toml —— 配方（由 JX3BOX manufacture 数据导出）
[[peifang]]
id = "cooking_437"
mingcheng = "云芝竹沥羹"
jiyi = "pengren"          # pengren 烹饪 / zhiyao 制药 / fengren 缝纫 / zhuzhao 铸造 / zijiang 梓匠
dengji = 50               # 配方等级要求（RequireLevel）
jingli = 75               # 精力消耗（CostVigor）
zhizuofei = 0             # 固定金币消耗（铜）
[[peifang.cailiao]]
wupin_id = "5_30853"
shuliang = 1
[[peifang.chuchan]]        # 产物槽（8 槽，带权重）
wupin_id = "5_89346"
min = 2
max = 3
quanzhong = 1006633       # 分母 2^20
# 精制品槽同理：wupin_id=5_89347, 权重 41943

# data/wuxingshi.toml —— 五行石转换（手工维护 + 待实测校准）
[[zhuanhuan]]
cong = "5_24429"          # 低阶五行石
dao = "5_24430"           # 高阶五行石
xuyao = 3                 # 每次投入数量
jingli = 10
jinbi = 3900              # 金币消耗（铜）
chenggonglv = 1.0         # 成功率（合成四级 2023 社区口径为 48.78%，2026 版本待校准）

# data/fuwuqi.toml —— 服务器（来自 JX3BOX/jx3box-data data/server）
[[fuwuqi]]
id = "jiandanqinxin"
mingcheng = "剑胆琴心"
daju = "电信区"

# data/values/live-130.toml —— 值集（可随数据通道下发）
[shouxufei]
shuilv_bp = 500           # 万分比：500 = 5%
quzheng = "half_up"       # half_up 四舍五入 / floor / ceil
baoguanfei = 8000         # 铜 / 24h
```

- **随包数据**：`data/wupin.toml`、`data/peifang.toml`、`data/wuxingshi.toml`、`data/fuwuqi.toml`、`data/values/`；
  运行期读取顺序：`JXCG_DATA_DIR` 环境变量 > exe 同目录 `data/`（测试用临时目录）。
- **用户数据**（不进仓库）：行情 `hangqing/{server}.toml`、快照 `hangqing/{server}/snapshots/*.toml`、`settings.toml`；
  放应用数据目录（修正 jpcg 读 CWD 的 `saved_config.toml` 的坑：数据目录 + 可选手动指定路径）。
- **数据版本**：`[version]{level, season, modified}` 与 `data_manifest.toml` 预留；界面显示当前数据版本。

### 7.1 第三方接口调研（2026-11-12 实测）

| 来源 | 能力 | 实测结论 | 采用 |
|------|------|----------|------|
| **JX3BOX 交易行** `next2.jx3box.com/api/item-price/{id}/detail?server=` | 逐条挂单：时间戳、数量、单价（铜/件），按服务器 | HTTPS、免 token、数据新鲜（当天）、单请求 ~1.2s、8 并发正常；无批量接口；`logs` 历史接口实测为空 | ✅ 默认行情源 |
| **JX3BOX 配方** `node.jx3box.com/manufactures?client=std&type={type}&mode=simple` + `/manufacture/{type}/{id}` | 5 技艺共 **1036 条**有效配方（烹饪 89 / 制药 62 / 缝纫 221 / 铸造 253 / 梓匠 411）；详情含材料 8 槽、产出 8 槽（权重分母 2^20）、精力、等级、分类 | HTTPS、免 token、目录+详情接口完整；详情需逐条拉取（构建期导出） | ✅ 构建期导出 TOML |
| **JX3BOX 物品/商店价** `/resource/std/item_merged.{id}`、`/craft/price?client=std`、`/manufacture/belong` | 物品名称/图标/描述；商店价表（49KB）；技艺分类 | 可用；商店价可作材料无行情时的兜底 | ✅ |
| **JX3BOX 服务器列表** `JX3BOX/jx3box-data` 仓库 `data/server/*.json` | 区服清单 | 可用，随数据打包 | ✅ |
| **hidewnd** `costing.hidewnd.cn/costing/{one,list}` | 单/多配方成本 + 交易行报价 + 精力 + 利润 | 接口活着，但**仅 HTTP**（HTTPS 证书错误）、无文档、无配方列表接口、依赖第三方数据 | ⚠️ 仅作口径参考，不接入 |
| **JX3API** `www.jx3api.com` | 金价/外观价/日常/沙盘等 | 商业 token（5000 次 ¥50 起）；不提供完整交易行挂单明细 | ❌ v0 不接 |
| [wind-jx3](https://github.com/hidewnd-winds/wind-jx3) | 社区开源（Apache-2.0）成本计算实现 + 接口/规则文档 | 提供 5% HALF_UP + 保管费 8000 铜、批量购买算法、概率口径等参考 | 参考口径（不复制代码） |

**接入原则**：行情请求全部在 Rust 侧（reqwest）→ 无 CORS 问题；缓存 TTL（默认 10 分钟）+ 并发上限 4–6 + 超时/重试 1 次 +
断网/失败自动降级到最后一次拉取价（last-known-good），无缓存再用手动价；显式「刷新行情」按钮 + 进度事件；不把 JX3BOX 数据二次分发给第三方。
**风险**：JX3BOX 是社区接口（无官方文档/SLA，可能变更或限流）→ provider 层抽象 `MarketProvider`，接口变了只换实现；README 标注数据来源。

## 8. 工程约定（对齐 jpcg）

- Rust edition 2024（rustc ≥ 1.85）；GPL-3.0；中文注释、拼音键名、`changes/` 逐条变更日志（同 jpcg Agreement）。
- **Makefile**：`setup`（bun install）/ `frontend` / `frontend-test` / `build` / `test` / `fmt-check` / `clippy` / `check-all` / `data-export` / `dev`；前端一律走 **bun**，不含 updater/modules。
- **CI**：rustfmt + clippy（不 -D warnings）+ `cargo test --workspace` + 前端 `bun install --frozen-lockfile` + `bun run build` + `bun run test`（CI 用 `oven-sh/setup-bun`）；
  三分支（dev/beta/release）与分支保护属于发布期事项，可后置（需 GitHub 仓库配合）。
- **版本**：root workspace version 从 `0.1.0` 起；数据 `[version]{level, season, modified}`；值集 id（如 `live-130`）。
- **测试**：core 金标准用例如 §6.4 口径（金额解析/格式化、5% HALF_UP 取整边界、保管费、保本价 ±1 铜、概率期望产出、缺价配方、排行稳定性）；
  市场层用录制的 JSON fixture 做 provider 解析测试；前端 vitest 覆盖 MoneyInput、结果卡、持久化。
- **窗口/包体**：`withGlobalTauri: true`、CSP 参考 jpcg；先出 macOS 本机可跑，Windows/Linux 打包放 P4。
- **数据合规**：README 标注数据来自 JX3BOX / 游戏数据；若上游反对，改为「运行时拉取」或「用户导入」模式（参考 jpcg PLAN M2）。

## 9. 里程碑

> 进度快照（2026-11-12）：P1 ✅ 骨架/核心计算；P2 ✅ 全量数据导出（1036 配方/1361 物品/22 服务器）+ 配方引擎；
> P3 ✅ 行情缓存/降级、Tauri 命令、前端四页、排行/设置全部完成（`make check-all` 全绿，debug 启动冒烟通过）；
> P4 进行中：五行石 UI、快照后端化、打包/更新通道待做。


| 阶段 | 内容 | 验收 |
|------|------|------|
| **P0 方案定稿** | 本文件 + 决策拍板 + API 调研 | 行情/费用口径确认，可开工 |
| **P1 骨架** | workspace + Tauri 壳 + 主题/导航/状态栏/Toast + 手动模式净收入/保本价 | `make dev` 起窗口，能算「挂 X 到手多少 / 保本价」；core 单测 + 前端测试通过 |
| **P2 数据与配方引擎** | `jx3box-export` 导出配方/物品/商店价/服务器 → `data/*.toml`；成本/利润/期望产出/排行引擎 + 金标准 + 计算页配方模式 | 选配方 → 材料自动带价 → 结果正确；金标准全绿 |
| **P3 行情与排行** | JX3BOX 行情拉取（缓存/节流/降级）+ 手动覆盖 + 行情页 + 快照趋势 + 排行页 + 设置页 | 一键刷新能拉到真实行情；断网时降级可用；排行可用 |
| **P4 打磨与工程化** | 五行石转换表、Makefile/CI/changes/README/AGENTS、打包 | `make check-all` 全绿；macOS 产物可双击运行 |

## 10. 风险与对策

| 风险 | 说明 | 对策 |
|------|------|------|
| 行情数据源 | JX3BOX 社区接口，无文档/SLA，可能限流或变更 | provider 抽象 + 缓存/节流/降级 + 手动覆盖兜底；接口变了只换实现 |
| 配方/物品数据 | 来源与再分发授权（参考 jpcg 数据授权议题） | 标注来源；数据与代码分离；必要时改为运行时拉取/用户导入 |
| 手续费口径 | 5%/HALF_UP/保管费 8000 铜来自社区实现，需实测 | 值集可改；上线前用游戏内一单校准并写测试 |
| 五行石规则 | manufacture API 不含五行石精炼，2026 版本规则待校准 | 独立转换表 + 可编辑；默认值标注来源与日期 |
| 精力机会成本 | 因人而异（搬砖效率不同） | 参数化「每点精力折合金」，结果里单列 |
| 游戏版本 | 130 级/新赛季数值会变 | 值集 + 数据版本号 + `data-export` 可重跑 |
| 录价效率 | 交易行列表不能复制（自动拉价解决大部分） | JX3BOX 一键刷新 + MoneyInput + 粘贴解析 + 最近使用 |
| 范围蔓延 | 万宝楼/倒卖/跨服容易越做越大 | v0 明确不做，列远期 |

## 11. 已确认（2026-11-12）

1. ✅ 行情：**联网自动拉价（JX3BOX）**，断网/失败使用最后一次拉取的价格；手动覆盖优先。
2. ✅ 费用：**5% 四舍五入（HALF_UP）+ 保管费 80 银/24h**，做成值集可改。
3. ✅ 五行石：**先内置社区规则默认值、可编辑**（标注来源与待校准）。
4. ✅ 名称/许可：**jxcg / GPL-3.0**；工程：**同技术栈裁剪版**（Tauri v2 + Rust core + React 19）。

## 12. 拍板后的分工（Agent Teams）

| 角色 | 写作用域（互不重叠） |
|------|--------------------|
| Lead | 根配置、`Cargo.toml` workspace、`Makefile`、CI、`AGENTS.md`/README、集成与终审、跑全量检查 |
| 前端 teammate | `examples/jxcg_app/src/**`（壳 + 页面 + 测试） |
| 核心 teammate | `crates/jxcg_api/**`、`crates/jxcg_core/**`（引擎 + 金标准测试） |
| 数据/市场 teammate | `crates/jxcg_market/**`、`server_tools/**`、`data/**`（JX3BOX 客户端 + 导出 + 首批数据） |

> 约定：先由 Lead 落 P1 骨架和数据 schema，再并行；每个改动附 `changes/` 一条。
