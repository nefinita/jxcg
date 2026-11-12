# JXCG — 剑网3 交易行计算器

与 [JPCG](../jpcg) 同风格的桌面工具：把「交易行行情 → 制作成本 → 税后收益 → 值不值得做」算清楚。

- **形态**：Tauri v2 + Rust workspace + React 19 + Vite（前端依赖用 **bun**）
- **视觉**：沿用 jpcg 的薄荷/松石绿设计 token、ActivityBar、StatusBar、Toast、深浅色与页面保活

## 快速开始

```sh
# 依赖（bun，不用 npm）
make setup            # cd examples/jxcg_app && bun install

# 开发：起 Tauri 窗口（Vite 端口 1420，与 jpcg 相同；同时跑两个项目时先停其中一个）
make dev

# 全量检查：fmt + clippy + cargo test + 前端 vitest + 前端构建
make check-all

# 打包 macOS .app（数据随包在 Contents/Resources/data）
cd examples/jxcg_app && bun run tauri build --bundles app
# 产物：target/release/bundle/macos/JXCG.app（未签名/未公证，首次打开用右键「打开」）

# 重新导出随包数据（JX3BOX，约 40s / 2400 次请求）
make data-export
```

> 本仓库的沙箱环境把 cargo/bun 缓存放工作区 `.cache/`；普通终端可忽略，直接 `cargo` / `bun` 即可。

## 功能（v0 已实现）

| 模块 | 内容 |
|------|------|
| 交易行净收入 | 金/银/铜（含砖）解析与格式化、5% 手续费（HALF_UP 到铜）、保管费 80 银/24h、到手价、保本价 |
| 配方成本利润 | 本赛季 590 条配方（烹饪 30 / 制药 20 / 缝纫 54 / 铸造 75 / 梓匠 411）：材料价优先级（行情 > NPC 商店价 > 缺价）、中间产物按自制成本摊入、精力成本、制作费、产物期望（2^20 权重）、净利润/利润率/每精力收益/保本单价、批量与挂单次数 |
| 五行石精炼 | 转换模型（低阶石 ×n + 精力 + 金币 + 成功率 → 高阶石，按期望成本算）；独立页面可选配方/改价/看净利润，缺价引导去行情页 |
| 中间产物 | 产物∩材料的物品（30 个：木板/百花布/药引/各类锭等）按「自身材料递归 + 精力 + 制作费」折算摊入最终产物；只产中间产物的配方默认不进排行（可切换） |
| 行情库 | JX3BOX 自动拉价（并发 4 / 15s 超时 / 失败重试 1 次）、**一键刷新全部相关物品（无 200 截断，约 850 个，带进度）**、断网回落 last-known-good、手动覆盖优先、粘贴导入、快照/趋势、JSON 导入导出 |
| 全配方排行 | 按 净利润 / 每精力利润 / 利润率 排序，缺价默认排除，可切换包含缺价；TOP 柱状图 |
| 设置 | 服务器、值集（费率/取整/保管费）、精力单价、技艺等级、自动拉价、主题、数据版本与重新加载 |

**数据**：随包 TOML（**590 配方 / 849 物品 / 22 服务器，共 360KB**），来源 JX3BOX，重新生成方式见 `data/README.md`。
**赛季过滤**：配方详情 `nDlc` = 资料片，当前 `max=9`（苍生铸世/130级）；导出默认保留 `nDlc=9` + 梓匠全部 + 旧中间材料闭包，剔除 446 条旧赛季配方（饺子皮/旧宴席/旧「风语·绝世」装备等）。
**口径**：5% 四舍五入 + 保管费 80 银/24h（来自社区开源实现交叉验证，实机校准后改 `data/values.toml` 即可）。

## 目录

```
crates/jxcg_api/      DTO 契约（Tauri 命令与前端类型的单一来源）
crates/jxcg_core/     纯计算：money/fee/craft/rank/convert/store（含金标准测试）
crates/jxcg_market/   JX3BOX 行情客户端 + 本地缓存 + last-known-good 降级
server_tools/jx3box-export/  构建期数据导出（配方/物品/商店价/服务器 → data/*.toml）
examples/jxcg_app/    React 前端 + src-tauri（Tauri 命令装配）
data/                 随包数据（TOML）
changes/              逐条变更日志
```

## 已知边界（P4）

- 值集自定义费率目前存 localStorage；快照/JSON 导入也走 localStorage + 逐条 upsert（后端命令未注册）。
- macOS `.app` 已可本地打包（未签名/未公证）；Windows 安装包、更新通道、签名公证未做。
- 五行石成功率仍为社区口径（页面已标注），实机校准后改 `data/wuxingshi.toml` 即可。
- 行情来自 JX3BOX 社区接口（无官方文档/SLA），接口变更时只需替换 `jxcg_market` 实现。

## 测试

- Rust：`cargo test --workspace`（api 契约 9 + core 70 + 金标准 7 + 真实数据冒烟 3 + market 21，另 1 个 ignored 为真联网冒烟）
- 前端：`cd examples/jxcg_app && bun run test`（13 文件 / 57 tests，含五页 smoke/刷新/中间产物/设置）

方案与决策见 [PLAN.md](PLAN.md)，开发约定见 [AGENTS.md](AGENTS.md)。
