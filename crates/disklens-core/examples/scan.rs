use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use disklens_core::{scan, ScanOptions};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("用法: cargo run --example scan -- <path>");
        std::process::exit(2);
    };
    let mut opts = ScanOptions::default();
    if args.any(|a| a == "--cross") {
        opts.cross_filesystems = true;
    }
    let path = PathBuf::from(path);

    match scan(&path, &opts) {
        Ok(tree) => {
            let s = &tree.stats;
            println!("root        {}", tree.root_path.display());
            println!("total bytes {}", tree.root_node().size);
            println!("entries     {}", s.entries);
            println!("errors      {}", s.errors);
            println!("elapsed ms  {}", s.elapsed_ms);
            println!("cancelled   {}", s.cancelled);
            if let Some(kb) = peak_rss_kb() {
                println!("peak rss kb {kb}");
            }
            let du = du_bytes(&tree.root_path, false);
            match du {
                Some(du) => {
                    let diff = tree.root_node().size as f64 - du as f64;
                    let pct = if du == 0 {
                        0.0
                    } else {
                        diff / du as f64 * 100.0
                    };
                    println!("du -sB1     {du}");
                    println!("vs du       {pct:+.3}%");
                }
                None => println!("du -sB1     不可用，跳过对照"),
            }
            // du 默认按已分配块计；逻辑大小对照用 --apparent-size
            if let Some(du) = du_bytes(&tree.root_path, true) {
                let pct = if du == 0 {
                    0.0
                } else {
                    (tree.root_node().size as f64 - du as f64) / du as f64 * 100.0
                };
                println!("du apparent {du}");
                println!("vs apparent {pct:+.3}%");
            }
        }
        Err(e) => {
            eprintln!("扫描失败: {e}");
            std::process::exit(1);
        }
    }
}

fn du_bytes(path: &std::path::Path, apparent: bool) -> Option<u64> {
    let mut cmd = Command::new("du");
    cmd.arg("-sB1");
    if apparent {
        cmd.arg("--apparent-size");
    }
    let out = cmd.arg(path).output().ok()?;
    // 权限不足时 du 会部分成功（exit 1）；仍可比对已扫到的大小
    let text = String::from_utf8_lossy(&out.stdout);
    u64::from_str(text.split_whitespace().next()?).ok()
}

/// Linux 峰值 RSS（VmHWM），避免依赖 GNU time。
fn peak_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        line.strip_prefix("VmHWM:")?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    })
}
