# data/ —— JXCG 随包数据

> 本目录由 `cargo run -p jx3box-export -- --out data` 生成（生成日期 2026-11-12，北京时间）。

## 来源与授权

- 数据来自 [JX3BOX](https://www.jx3box.com) 社区接口（非官方、无 SLA）与游戏数据；仅本地计算使用，不再分发。
- 配方：`node.jx3box.com/manufactures` + `manufacture/{type}/{id}`；物品：`resource/std/item_merged.{id}`；商店价：`craft/price`；服务器：`JX3BOX/jx3box-data` 仓库 `data/server/*.json`。
- 若上游反对再分发，改为「运行时拉取 / 用户导入」模式（见 PLAN.md §10）。

## 本次生成统计

- 配方：**1036** 条（烹饪 89 / 制药 62 / 缝纫 221 / 铸造 253 / 梓匠 411）
- 物品：**1361** 条（cailiao 394 / chengpin 961 / xiaohao 0 / wuxingshi 6）；其中有 NPC 商店价 74 条
- 服务器：22 个
- 网络：请求 2405 次 / 缓存命中 0 次 / 最终失败 0 次；配方详情失败 0 条，物品缺失 0 条；耗时 40.4s

## 文件

| 文件 | 内容 | 生成方式 |
|------|------|----------|
| `catalog.toml` | 数据版本 `[banben]` | 导出（版本号手工维护/计划校准） |
| `wupin.toml` | 物品（名称/分类/商店价） | 导出 |
| `peifang.toml` | 生活技艺配方（材料 8 槽 + 产物 8 槽权重） | 导出 |
| `fuwuqi.toml` | 服务器（id/mingcheng/daju） | 导出 |
| `wuxingshi.toml` | 五行石转换 | **手工维护**（待实机校准） |
| `values.toml` | 值集（费率/取整/保管费） | **手工维护**（默认 5% HALF_UP + 8000 铜/24h） |

## 口径与已知问题

- 产物权重分母固定 `2^20 = 1048576`（普通 1006633 / 精制 41943）；材料槽只保留数量>0，产物槽只保留权重>0。
- 配方 id 形如 `cooking_437`（`{JX3BOX tab}_{游戏 ID}`）；`jiyi` 映射：cooking→pengren、medicine→zhiyao、tailoring→fengren、founding→zhuzhao、furniture→zijiang。
- 物品 `pinlei` 为**粗分**（cailiao/chengpin/xiaohao/wuxingshi）：配方产物 → chengpin；五行石 → wuxingshi；其余按上游 `TypeLabel`，拿不准按 cailiao 并标 `laiyuan=jx3box(粗分)`。
- `shangdianjia` 只取 NPC 售卖价（`craft/price`，仅 77 条）；物品表 `Price` 是卖给商店的价格，不能当成本兜底，未采用。
- `dui`（一组堆叠数）上游未提供，暂缺省；如需按组购买请在设置里补默认值。
- 五行石精炼规则不在 `manufacture` 接口里，`wuxingshi.toml` 由手工维护，等待实机校准。
- 服务器 `id` 为手工维护的 pinyin 映射（`util::pinyin_id`）；**JX3BOX 行情接口的 `server` 参数必须用中文名 `mingcheng`**。

### 生成提示/告警

- 保留现有 values.toml（手工维护的值集，导出不覆盖）
- 保留现有 wuxingshi.toml（手工维护的五行石规则，待实机校准）

## 重新生成

```sh
# 需要联网；对上游礼貌（并发 5、超时 15s、失败重试 1 次）
cargo run -p jx3box-export -- --out data
# 调试：只抓每类前 N 条，并把原始 JSON 缓存到 .cache/jx3box
cargo run -p jx3box-export -- --out data --limit 5 --cache .cache/jx3box
```
