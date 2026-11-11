# JXCG — 剑网3 交易行计算器

与 [JPCG](../jpcg) 同风格的桌面工具：把「交易行行情 → 制作成本 → 税后收益 → 值不值得做」算清楚。

- **形态**：Tauri v2 + Rust workspace + React 19 + Vite（bun 管理前端依赖）
- **功能（v0）**：交易行净收入/保本价、生活技艺配方成本利润、五行石精炼、行情库/快照、全配方收益排行
- **数据**：配方/物品/服务器随包 TOML；行情从 JX3BOX 自动拉取（断网回落到最后一次缓存价），可手动覆盖
- **口径**：交易行手续费 5%（四舍五入到铜）+ 保管费 80 银/24h，做成值集可改

方案与决策见 [PLAN.md](PLAN.md)，开发约定见 [AGENTS.md](AGENTS.md)。

```sh
make setup        # bun install
make dev          # Tauri 开发模式
make check-all    # fmt + clippy + 测试 + 构建
```
