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

- **金额**：`jxcg_api::Tong = i64`，单位「铜」；`1 金 = 100 银 = 10_000 铜`，`1 砖 = 10_000 金`。禁止浮点金额。
- **费用口径**：手续费 5%（`shuilv_bp = 500`，默认 HALF_UP 到铜）+ 保管费 80 银/24h（`baoguanfei = 8000`）；
  全部在值集 `data/values.toml`，界面可改。依据见 PLAN.md §6.2。
- **产出概率**：权重分母 `QUANZHONG_FENMU = 1_048_576`（2^20）；普通/精制 = 1006633/41943。
- **数据目录**：`JXCG_DATA_DIR` > exe 同目录 `data/` > debug 时仓库 `data/`。
- **用户数据**：行情/快照/设置放应用数据目录，不进仓库（`.gitignore` 已排除 `hangqing/`、`settings.toml`）。
- **行情源**：JX3BOX `https://next2.jx3box.com/api/item-price/{id}/detail?server=`（非官方、无 SLA）。
  请求统一在 `jxcg_market`，缓存/并发上限/失败降级（last-known-good）在 P3 实现。
- **Tauri commands**：`calculate_net` / `calculate_baoben` / `calculate_recipe` / `rank_recipes` / `calculate_convert` /
  `load_catalog` / `load_prices` / `upsert_price` / `refresh_prices` / `load_settings` / `save_settings` / `ping`。
- **前端**：React 19 + TS + Vite + CSS Modules；`window.__TAURI__` 调用，浏览器下走 `api/commands.ts` 的 mock。
- **Tauri 配置**：`withGlobalTauri: true`、CSP `null`、`frontendDist: ../dist`、`beforeDevCommand: bun run dev`。
- **库代码禁止 `unwrap/expect`**：统一 `Result<_, JxcgError>`，Tauri 命令返回中文错误串。

## Data schema（TOML）

- `data/catalog.toml`：`[banben] level/season/modified/note`
- `data/wupin.toml`：`[[wupin]] id/mingcheng/pinlei/dui?/shangdianjia?/laiyuan?`
- `data/peifang.toml`：`[[peifang]] id/mingcheng/jiyi/dengji/jingli/zhizuofei?/[[cailiao]]/[[chuchan]]`
  - `jiyi` ∈ `pengren|zhiyao|fengren|zhuzhao|zijiang`
  - `[[chuchan]]` 槽：`wupin_id/min/max/quanzhong`（权重分母 2^20）
- `data/wuxingshi.toml`：`[[zhuanhuan]] id/mingcheng/cong/dao/xuyao/jingli/jinbi/chenggonglv_bp`
- `data/fuwuqi.toml`：`[[fuwuqi]] id/mingcheng/daju?`
- `data/values.toml`：`[[zhiji]] id/mingcheng/note/[zhiji.shouxufei] shuilv_bp/quzheng/baoguanfei`

## 维护流程

- 每个改动附 `changes/YYYY-MM-DD-HHMMSS.md`（什么/为什么/关键决策/验证）。
- 提交风格：`type(scope): 中文描述`（feat/fix/refactor/test/docs/build/chore）。
- 改动前先跑 `make test`（或相关 crate 单测）；合并前 `make check-all`。
- 计算口径变更必须同时更新 PLAN.md §6 与金标准测试。
