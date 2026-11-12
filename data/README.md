# data/ —— JXCG 随包数据

> 本目录由 `cargo run -p jx3box-export -- --out data` 生成（生成日期 2026-11-12，北京时间）。

## 来源与授权

- 数据来自 [JX3BOX](https://www.jx3box.com) 社区接口（非官方、无 SLA）与游戏数据；仅本地计算使用，不再分发。
- 配方：`node.jx3box.com/manufactures` + `manufacture/{type}/{id}`；物品：`resource/std/item_merged.{id}`；商店价：`craft/price`；服务器：`JX3BOX/jx3box-data` 仓库 `data/server/*.json`。
- 若上游反对再分发，改为「运行时拉取 / 用户导入」模式（见 PLAN.md §10）。

## 本次生成统计

- 赛季过滤：**nDlc=9**；保留 590 条 / 剔除 446 条旧赛季配方（下详）
- 配方：**590** 条（烹饪 30 / 制药 20 / 缝纫 54 / 铸造 75 / 梓匠 411）
- 物品：**849** 条（cailiao 210 / chengpin 636 / xiaohao 0 / wuxingshi 3）；其中有 NPC 商店价 46 条
- 中间产物：**30** 个（产物 ∩ 材料，core 按自制成本展开）
- 服务器：22 个
- 网络：本次复用原始 JSON 缓存（`--cache`，命中 1893 条 URL），未联网；要同步最新数据请去掉 `--cache` 重跑。

## 赛季过滤（nDlc）

当前正式服资料片 **nDlc=9（苍生铸世）**；导出器只保留「本赛季用得上的配方」，剔除已完成资料片的旧配方。

规则（`server_tools/jx3box-export/src/season.rs`）：

1. `nDlc == 9` 的当前资料片配方：**154** 条；
2. 梓匠（`furniture`）全部保留：**411** 条（家具配方无 nDlc，晋秩仍要木板/卯榫/雕花等）；
3. 旧中间材料闭包补齐：**25** 条（产物被保留配方当材料的旧配方，迭代到不动点）。
   - 闭包配方：卤料 / 酱料 / 馅料 / 安营炊鼎 / 药引 / 药丸 / 药膏 / 定魂丹砂 / 百花布 / 蜀染布 / 五色石 / 银丝缎 / 七彩线 / 同袍营幡 / 熟铁锭 / 密银锭 / 黄铜锭 / 青铜锭 / 锡锭 / 镔铁锭 / 生铁锭 / 铅锭 / 定营铁桩 / 青石锭 / 镀金锭
   - 派生中间产物（30 个）：木板(5_35712) / 卯榫(5_35713) / 雕花(5_35714) / 篾条(5_35715) / 鱼胶(5_35716) / 百花布(5_89956) / 蜀染布(5_89957) / 五色石(5_89958) / 银丝缎(5_89959) / 七彩线(5_89960) / 熟铁锭(5_89961) / 密银锭(5_89962) / 黄铜锭(5_89963) / 青铜锭(5_89964) / 锡锭(5_89965) / 镔铁锭(5_89966) / 生铁锭(5_89967) / 铅锭(5_89968) / 馅料(5_89969) / 酱料(5_89970) / 卤料(5_89971) / 药引(5_89972) / 药丸(5_89973) / 药膏(5_89974) / 定营铁桩(5_90032) / 同袍营幡(5_90033) / 安营炊鼎(5_90034) / 定魂丹砂(5_90035) / 青石锭(5_90436) / 镀金锭(5_90437)
   - 保留（按技艺）：烹饪 30 / 制药 20 / 缝纫 54 / 铸造 75 / 梓匠 411
   - 剔除（按技艺）：烹饪 59 / 制药 42 / 缝纫 167 / 铸造 178 / 梓匠 0

已剔除（446 条）示例：饺子皮 / 饺子馅 / 炼狱水煮鱼 / 佛跳墙 / 转神餐 / 咸鱼 / 辣酱肉片 / 如意饺 / 戏凤饺 / 盘龙饺 / 百味宴 / 山河宴 / 汉宫宴 / 南山宴 / 东海宴 / 任侠宴 / 兄弟宴 / 贵妃宴 / 欢庆宴 / 三头宴。

- 切回全量（含旧赛季，仅调试/对比）：`cargo run -p jx3box-export -- --out data --all`
- 指定资料片：`--season 9`（数字是 nDlc）；默认 `--season auto` = max(nDlc)。

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
- `shangdianjia` 只取 NPC 售卖价（`craft/price`）；物品表 `Price` 是卖给商店的价格，不能当成本兜底，未采用。
- `dui`（一组堆叠数）上游未提供，暂缺省；如需按组购买请在设置里补默认值。
- 五行石精炼规则不在 `manufacture` 接口里，`wuxingshi.toml` 由手工维护，等待实机校准。
- 服务器 `id` 为手工维护的 pinyin 映射（`util::pinyin_id`）；**JX3BOX 行情接口的 `server` 参数必须用中文名 `mingcheng`**。

### 生成提示/告警

- 保留现有 values.toml（手工维护的值集，导出不覆盖）
- 保留现有 wuxingshi.toml（手工维护的五行石规则，待实机校准）

## 重新生成

```sh
# 需要联网；对上游礼貌（并发 5、超时 15s、失败重试 1 次）
# 默认 --season auto = 当前资料片 max(nDlc)，剔除旧赛季配方
cargo run -p jx3box-export -- --out data
# 全量导出（含旧赛季配方，仅调试/对比）
cargo run -p jx3box-export -- --out data --all
# 指定资料片 nDlc（例如 9 = 苍生铸世）
cargo run -p jx3box-export -- --out data --season 9
# 调试/续跑：复用 .cache/jx3box 原始 JSON，只抓每类前 N 条
cargo run -p jx3box-export -- --out data --limit 5 --cache .cache/jx3box
```
