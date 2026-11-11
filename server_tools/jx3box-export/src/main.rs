// ============================================================================
// jx3box-export —— 构建期数据导出（P2）
//
// 从 JX3BOX 抓取 5 技艺配方目录/详情、物品、商店价、服务器列表，导出为 data/*.toml。
// 具体实现在 P2 由 market-data teammate 补齐；当前仅打印帮助，保证 workspace 可编译。
// ============================================================================

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("jx3box-export（骨架）：参数 = {args:?}");
    println!("用法：cargo run -p jx3box-export -- --out data");
    println!("P2 待实现：配方目录/详情、物品、商店价、服务器 → data/*.toml");
}
