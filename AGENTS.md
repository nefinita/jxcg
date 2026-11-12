# JXCG — JX3 交易行计算器

Rust workspace（edition 2024）+ Tauri v2 + React 19 + Vite 的桌面应用。
前端包管理器为 **bun**（不用 npm）。文案/注释/键名沿用 jpcg 风格：中文注释、拼音键名、金额整数铜。

## Workspace

| Path | Role |
|------|------|
| `crates/jxcg_api/` | 纯 DTO 契约（serde，零业务逻辑）——Tauri 命令与前端类型的单一来源 |
| `crates/jxcg_core/` | 纯计算：money/fee/craft/rank/convert/store（不联网、不依赖 Tauri） |
| `crates/jxcg_market/` | 行情 provider：JX3BOX 客户端 + 并发/超时（HTTP 只在这里） |
| `examples/jxcg_app/` | React 19 + TS + Vite 前端（bun.lock 管理依赖） |
| `examples/jxcg_app/src-tauri/` | Tauri v2 壳与命令装配（`commands/{calc,data,market,settings}.rs`） |
| `server_tools/jx3box-export/` | 构建期数据导出（P2）：配方/物品/商店价/服务器 → `data/*.toml` |
| `data/` | 随包 TOML（catalog/wupin/peifang/wuxingshi/fuwuqi/values） |
| `changes/` | 每次改动一条 `YYYY-MM-DD-HHMMSS.md`（同 jpcg Agreement） |

## 环境（沙箱 / 缓存）

本机沙箱禁止写 `$HOME`，cargo 与 bun 缓存统一放工作区 `.cache/`：

```sh
export CARGO_HOME="$PWD/.cache/cargo"          # 已从 ~/.cargo 复制 cache+index（约 330MB，不入库）
export BUN_INSTALL_CACHE_DIR="$PWD/.cache/bun" # bun 安装缓存
```

- 直接用 `cargo ...` 前必须先 export `CARGO_HOME`；走 `make` 目标时 Makefile 已自动导出。
- `.cache/` 已在 `.gitignore` 中；不要把它提交。

## Commands

```sh
make setup          # 前端依赖：bun install --frozen-lockfile
make frontend       # tsc -b && vite build
make frontend-test  # vitest
make build          # cargo build --workspace
make dev            # bun run tauri dev
make test           # cargo test --workspace
make check-all      # fmt-check + clippy + build + test + frontend-test + frontend
make data-export    # P2：jx3box-export 重新导出 data/
```

## Key facts

- **版本**：苍生铸世 50 级（2026-10-29 等级回退）；数据 `[banben].level = 50`，生活技艺等级上限 50；值集 id `live-130` 为历史兼容名。
- **金额**：`jxcg_api::Tong = i64`，单位「铜」；`1 金 = 100 银 = 10_000 铜`，`1 砖 = 10_000 金`。禁止浮点金额。
- **费用口径**：手续费 5%（`shuilv_bp = 500`，默认 HALF_UP 到铜）+ 保管费 80 银/24h（`baoguanfei = 8000`）；
  全部在值集 `data/values.toml`，界面可改。依据见 PLAN.md §6.2。
- **产出概率**：权重分母 `QUANZHONG_FENMU = 1_048_576`（2^20）；普通/精制 = 1006633/41943。
- **取价口径（统一 `crates/jxcg_core/src/price.rs`）**：**手填（`shoudong`）> NPC 商店价（`shangdianjia > 0`）> 行情（`jiage`）> 缺价**；
  NPC 有售材料（料酒/今朝醉/药料等，当前 44 个）不用交易行价，不做取低；craft/rank/convert/wuxingshi 全部复用。
- **五行石合成**：`data/wuxingshi.toml` 分 `[[fangan]]`（方案）+ `[[hecheng]]`（单步：材料/精力/加工费/概率 bp/万分比）；
  一键合成到六级 = 面板配方递归期望（成功率 1/p 放大、副产物折价冲抵、8 级 `baoji_bp` 可调默认 0）；命令 `calculate_wuxingshi`。

- **数据赛季过滤**：配方详情 `nDlc` = 资料片版本，当前正式服 `max=9`（苍生铸世，**50 级**，2026-10-29 等级回退）。
  导出默认只保留「本赛季用得上」的配方：`nDlc==9` + 梓匠 5_ 产物配方（10_ 旧家园家具成品剔除）+ 旧中间材料闭包；
  当前数据 279 配方 / 468 物品（含 29 个中间产物、五行石 1–8 级），累计剔除 757 条（旧赛季 446 + 旧家园家具成品 311）。
  重导出：`make data-export`（`--all` 可回退全量，`--season N` 指定资料片）；见 `data/README.md`。

- **中间产物**：同时是某配方产物与某配方材料的物品（Catalog 派生，当前 29 个；数据已按本赛季过滤）；成本按自制展开
  （材料递归 + 精力×精力单价 + 制作费，按期望产出折算）摊入最终产物；只产中间产物的配方默认不进入排行
  （`baohan_zhongjian` 可切换）。开关 `zhongjian_zizhi` 在 JisuanRequest/PaihangRequest/Settings，默认 true，
  关闭时回退「按行情价」。DTO 字段：`JisuanResult.{chengben_zhongjian,jingli_zhongjian,zhongjian[]}`、
  `CailiaoMingxi.zhongjian`、`PaihangRow.zhongjian`；
  `JisuanResult.danjia_chengben`（单件成本 = 总成本/期望产出）、`JisuanResult.bukede`（无行情/无 NPC 价/无配方的不可获得材料）。

- **数据目录**：`JXCG_DATA_DIR`（无效即报错）> Tauri `resource_dir()/data`（打包后的 .app/安装包）> exe 同目录 `data/` > debug 时仓库 `data/`；见 `src-tauri/src/state.rs`。
- **用户数据**：行情/快照/设置放应用数据目录，不进仓库（`.gitignore` 已排除 `hangqing/`、`settings.toml`）。
- **行情源**：JX3BOX `https://next2.jx3box.com/api/item-price/{id}/detail?server=`（非官方、无 SLA）。
  请求统一在 `jxcg_market`，缓存/并发上限/失败降级（last-known-good）在 P3 实现。
- **Tauri commands**：`calculate_net` / `calculate_baoben` / `calculate_recipe` / `rank_recipes` / `calculate_convert` / `calculate_wuxingshi` /
  `load_catalog` / `reload_catalog` / `load_prices` / `upsert_price` / `refresh_prices` / `load_settings` / `save_settings` / `ping`。
- **前端**：React 19 + TS + Vite + CSS Modules；`window.__TAURI__` 调用，浏览器下走 `api/commands.ts` 的 mock。
- **行情刷新**：Tauri 命令 `refresh_prices` 通过事件 `refresh-progress` 回传 `RefreshProgress { done, total, failed }`；
  JX3BOX 的 server 参数必须是**服务器中文名**（`catalog.fuwuqi[].mingcheng`），传 pinyin id 会被误判缺价；服务器 id 用完整 pinyin（如 `changancheng`）。
- **目录缓存**：`AppState` 进程内缓存 `Arc<Catalog>`（启动后 data/ 不变），`reload_catalog` 清缓存重载。
- **Tauri 配置**：`withGlobalTauri: true`、CSP `null`、`frontendDist: ../dist`、`beforeDevCommand: bun run dev`。
- **库代码禁止 `unwrap/expect`**：统一 `Result<_, JxcgError>`，Tauri 命令返回中文错误串。

## Data schema（TOML）

- `data/catalog.toml`：`[banben] level/season/modified/note`
- `data/wupin.toml`：`[[wupin]] id/mingcheng/pinlei/dui?/shangdianjia?/laiyuan?`
- `data/peifang.toml`：`[[peifang]] id/mingcheng/jiyi/dengji/jingli/zhizuofei?/[[cailiao]]/[[chuchan]]`
  - `jiyi` ∈ `pengren|zhiyao|fengren|zhuzhao|zijiang`
  - `[[chuchan]]` 槽：`wupin_id/min/max/quanzhong`（权重分母 2^20）
- `data/wuxingshi.toml`（手工维护）：`[[zhuanhuan]] id/mingcheng/cong/dao/xuyao/jingli/jinbi/chenggonglv_bp`（旧单步）
  - `[[fangan]] id/mingcheng/note`；`[[hecheng]] id/fangan/chanpin/jingli/jiagongfei/[[cailiao]]/[[jieguo]]{wupin_id,bp,shuliang}`
  - 八级（连升两级）在 `mianban_6` 里预留 `bp=0` 槽，待实机校准；请求可传 `baoji_bp` 覆盖。
- `data/fuwuqi.toml`：`[[fuwuqi]] id/mingcheng/daju?`
- `data/values.toml`：`[[zhiji]] id/mingcheng/note/[zhiji.shouxufei] shuilv_bp/quzheng/baoguanfei`

## 开源与仓库

- 仓库：[nefinita/jxcg](https://github.com/nefinita/jxcg)（GitHub 组织 nefinita），**GPL-3.0**，见 `LICENSE`。
- 分支模型对齐 jpcg：`dev`（默认/集成）⊂ `beta`（公测）⊂ `release`（稳定）；`dev`/`beta`/`release` 受保护，
  要求 PR + 1 review + CODEOWNERS + 线性历史 + 指定 CI 检查（Rust 构建 + 测试 + lint / 前端构建 (tsc + vite) / 金标准回归（计算数值））。
- 每次推送前跑 `make check-all`；数据随仓库分发（来源见 `data/README.md`）。

## 维护流程

- 每个改动附 `changes/YYYY-MM-DD-HHMMSS.md`（什么/为什么/关键决策/验证）。
- 提交风格：`type(scope): 中文描述`（feat/fix/refactor/test/docs/build/chore）。
- 改动前先跑 `make test`（或相关 crate 单测）；合并前 `make check-all`。
- 计算口径变更必须同时更新 PLAN.md §6 与金标准测试。
