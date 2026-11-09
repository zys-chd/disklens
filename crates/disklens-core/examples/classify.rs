//! 扫描 ~/Projects，打印安全等级汇总与 Top 10 可清理目录。
//! 运行：cargo run -p disklens-core --example classify

use std::time::Instant;

use disklens_core::{
    classify_tree, default_rules_path, node_path, safety_stats, scan, NodeKind, RuleSet,
    SafetyLevel, ScanOptions, NO_PARENT,
};

fn fmt_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.2} {}", UNITS[u])
}

fn main() {
    let home = dirs::home_dir().expect("找不到 home 目录");
    let root = home.join("Projects");
    println!("扫描 {} …", root.display());
    let mut tree = scan(&root, &ScanOptions::default()).expect("扫描失败");

    let rs = RuleSet::load_or_create(&default_rules_path(), &home);
    println!(
        "规则 {} 条（{}），扫描节点 {}，用时 {} ms",
        rs.len(),
        default_rules_path().display(),
        tree.nodes.len(),
        tree.stats.elapsed_ms
    );

    let t = Instant::now();
    classify_tree(&mut tree, &rs);
    let rep = safety_stats(&tree, &rs);
    println!("分类+统计耗时 {:?}\n", t.elapsed());

    println!("== 各等级（按 category+level 分组，嵌套同组只计一次）==");
    let mut per_level = [0u64; 4];
    for s in &rep.stats {
        let i = s.level.order() as usize;
        per_level[i] += s.bytes;
    }
    for lvl in [
        SafetyLevel::Safe,
        SafetyLevel::Careful,
        SafetyLevel::Unknown,
        SafetyLevel::Keep,
    ] {
        let bytes = match lvl {
            SafetyLevel::Safe => rep.safe_bytes,
            SafetyLevel::Careful => rep.careful_bytes,
            _ => per_level[lvl.order() as usize],
        };
        println!("{:<12} {:>12}", lvl.label(), fmt_bytes(bytes));
    }
    println!(
        "\n🟢 可回收 {} ｜ 🟡 谨慎重建 {}",
        fmt_bytes(rep.safe_bytes),
        fmt_bytes(rep.careful_bytes)
    );

    println!("\n== Top 8 分类分组 ==");
    for s in rep.stats.iter().take(8) {
        println!(
            "{:<12} {:<14} {:>10}  {:>7} 个目录",
            s.category,
            s.level.label(),
            fmt_bytes(s.bytes),
            s.count
        );
    }

    let mut clean: Vec<(u64, usize)> = (0..tree.nodes.len())
        .filter_map(|i| {
            let n = &tree.nodes[i];
            if n.kind != NodeKind::Dir {
                return None;
            }
            let s = tree.safety[i];
            if !matches!(s.level, SafetyLevel::Safe | SafetyLevel::Careful) {
                return None;
            }
            let rule = rs.rule_at(s.rule_idx()?)?;
            if rule.needs_root {
                return None; // 需 sudo：只给命令，不列入可清理
            }
            if n.parent != NO_PARENT {
                let ps = tree.safety[n.parent as usize];
                if ps.level == s.level && rs.category(ps) == rs.category(s) {
                    return None; // 已包含在父目录里
                }
            }
            Some((n.size, i))
        })
        .collect();
    clean.sort_unstable_by_key(|x| std::cmp::Reverse(x.0));

    println!("\n== Top 10 可清理目录（safe/careful，无需 root）==");
    for (size, id) in clean.iter().take(10) {
        let s = tree.safety[*id];
        let rule = rs.rule_at(s.rule_idx().unwrap()).unwrap();
        println!(
            "{:>10}  {:<10} {}",
            fmt_bytes(*size),
            s.level.label(),
            node_path(&tree, *id as u32).display()
        );
        println!("            └ {}（{}）", rule.reason, rule.method);
    }
}
