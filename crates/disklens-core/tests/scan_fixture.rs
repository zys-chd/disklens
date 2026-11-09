mod support;

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use disklens_core::{scan, scan_with, NodeKind, ScanError, ScanOptions, Tree};
use support::*;
use tempfile::TempDir;

fn basic_fixture() -> (TempDir, Tree) {
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("a/a1.bin"), 100);
    write(&root.join("a/a2.bin"), 200);
    write(&root.join("a/aa/a3.bin"), 300);
    write(&root.join("b/b1.bin"), 400);
    write(&root.join("b/bb/b2.bin"), 500);
    dir(&root.join("c"));
    let tree = scan_path(root);
    (td, tree)
}

#[test]
fn three_level_three_dirs_sizes_match_exactly() {
    let (_td, tree) = basic_fixture();
    assert_eq!(tree.root_node().size, 1500);
    assert_eq!(tree.root_node().file_count, 5);
    assert_eq!(tree.root_node().dir_count, 5);
    assert_eq!(find(&tree, &["a"]).size, 600);
    assert_eq!(find(&tree, &["a"]).file_count, 3);
    assert_eq!(find(&tree, &["a"]).dir_count, 1);
    assert_eq!(find(&tree, &["a", "aa"]).size, 300);
    assert_eq!(find(&tree, &["b"]).size, 900);
    assert_eq!(find(&tree, &["b", "bb"]).file_count, 1);
    assert_eq!(find(&tree, &["c"]).size, 0);
    assert_eq!(find(&tree, &["c"]).dir_count, 0);
    assert_eq!(tree.stats.entries, 11); // root + 5 dirs + 5 files
    assert_eq!(tree.stats.errors, 0);
    tree.validate().unwrap();
}

#[test]
fn aggregation_invariants_hold_on_fixture() {
    let (_td, tree) = basic_fixture();
    // I1/I2 已由 validate 全量检查；这里再明确断言一次根节点公式
    let root = tree.root_node();
    let child_sum: u64 = root.children.iter().map(|&c| tree.node(c).size).sum();
    assert_eq!(root.size, root.own_size + child_sum);
    assert_eq!(root.own_size, 0);
    let direct_files = root
        .children
        .iter()
        .filter(|&&c| tree.node(c).kind == NodeKind::File)
        .count() as u32;
    let nested: u32 = root
        .children
        .iter()
        .filter(|&&c| tree.node(c).kind == NodeKind::Dir)
        .map(|&c| tree.node(c).file_count)
        .sum();
    assert_eq!(root.file_count, direct_files + nested);
    tree.validate().unwrap();
}

#[cfg(unix)]
#[test]
fn hardlink_dedup_counts_once_not_twice() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    let a = root.join("A/f1.bin");
    write(&a, 10 * 1024);
    dir(&root.join("B"));
    fs::hard_link(&a, root.join("B/f2.bin")).unwrap();
    let tree = scan_path(root);

    assert_eq!(tree.root_node().size, 10 * 1024);
    assert_eq!(tree.root_node().file_count, 2);
    let total = find(&tree, &["A"]).size + find(&tree, &["B"]).size;
    assert_eq!(total, 10 * 1024);
    assert!(find(&tree, &["A"]).size == 10 * 1024 || find(&tree, &["B"]).size == 10 * 1024);
    tree.validate().unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_is_not_followed_and_has_zero_size() {
    use std::os::unix::fs::symlink;
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("real/target/data.bin"), 1000);
    symlink("real/target", root.join("linkdir")).unwrap();
    symlink("real/target/data.bin", root.join("linkfile")).unwrap();
    let tree = scan_path(root);

    assert_eq!(tree.root_node().size, 1000);
    assert_eq!(find(&tree, &["linkdir"]).kind, NodeKind::Symlink);
    assert_eq!(find(&tree, &["linkdir"]).size, 0);
    assert!(find(&tree, &["linkdir"]).children.is_empty());
    assert_eq!(find(&tree, &["linkfile"]).size, 0);
    assert_eq!(find(&tree, &["real"]).size, 1000);
    tree.validate().unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_cycle_scan_terminates_without_hang() {
    use std::os::unix::fs::symlink;
    let td = TempDir::new().unwrap();
    let root = td.path();
    dir(&root.join("a"));
    dir(&root.join("b"));
    symlink("../b", root.join("a/to_b")).unwrap();
    symlink("../a", root.join("b/to_a")).unwrap();
    let tree = scan_path(root);
    assert_eq!(tree.root_node().size, 0);
    assert_eq!(tree.root_node().file_count, 0);
    assert_eq!(count_kind(&tree, NodeKind::Symlink), 2);
    assert_eq!(tree.stats.entries, 5);
    tree.validate().unwrap();
}

#[cfg(unix)]
#[test]
fn permission_denied_dir_reports_error_without_panic() {
    use std::os::unix::fs::PermissionsExt;
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("locked/secret.bin"), 1234);
    write(&root.join("ok/good.bin"), 456);
    let locked = root.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    if fs::read_dir(&locked).is_ok() {
        // 以 root 跑测试时权限位无效，跳过
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skip: 当前用户可读 000 目录");
        return;
    }
    let tree = scan_path(root);
    assert_eq!(tree.root_node().size, 456);
    assert_eq!(tree.stats.errors, 1);
    assert_eq!(find(&tree, &["locked"]).error_count, 1);
    assert_eq!(find(&tree, &["locked"]).size, 0);
    assert_eq!(find(&tree, &["ok", "good.bin"]).size, 456);
    tree.validate().unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn non_utf8_filename_does_not_panic_and_uses_lossy_display() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let td = TempDir::new().unwrap();
    let root = td.path();
    let name = OsStr::from_bytes(b"bad-\xff\xfe.bin");
    write(&root.join(name), 7);
    let tree = scan_path(root);

    assert_eq!(tree.root_node().size, 7);
    let node = tree
        .nodes
        .iter()
        .find(|n| n.name.starts_with("bad-"))
        .unwrap();
    assert!(node.name.contains('\u{FFFD}'), "name={:?}", node.name);
    assert!(node.name.ends_with(".bin"));
    let stats = disklens_core::type_stats(&tree, 10);
    assert_eq!(stats[0].ext, "bin");
    assert_eq!(stats[0].files, 1);
    assert_eq!(
        disklens_core::node_path(&tree, tree.root).to_string_lossy(),
        root.to_string_lossy()
    );
    tree.validate().unwrap();
}

#[cfg(unix)]
#[test]
fn unicode_and_255_byte_names_are_preserved() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    let unicode_dir = "中文 目录📦";
    let long_name = "x".repeat(255);
    write(&root.join(unicode_dir).join(&long_name), 1);
    write(&root.join(unicode_dir).join("数据.txt"), 12);
    let tree = scan_path(root);

    assert_eq!(find(&tree, &[unicode_dir]).size, 13);
    assert_eq!(find(&tree, &[unicode_dir, &long_name]).size, 1);
    assert_eq!(find(&tree, &[unicode_dir, "数据.txt"]).size, 12);
    assert_eq!(
        disklens_core::node_path(&tree, find_id(&tree, &[unicode_dir, "数据.txt"])),
        root.join(unicode_dir).join("数据.txt")
    );
    tree.validate().unwrap();
}

#[test]
fn zero_byte_and_sparse_file_use_logical_size() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("empty.bin"), 0);
    sparse(&root.join("sparse.bin"), 1 << 30);
    let tree = scan_path(root);
    assert_eq!(find(&tree, &["empty.bin"]).size, 0);
    assert_eq!(find(&tree, &["sparse.bin"]).size, 1 << 30);
    assert_eq!(tree.root_node().size, 1 << 30);
    assert_eq!(tree.root_node().file_count, 2);
    tree.validate().unwrap();
}

/// shortcut: Windows 传统 MAX_PATH(260) 装不下 40 层临时目录，这里用 24 层
/// （仍远超日常深度）；要回到 40 层需 CI 打开长路径支持。
#[test]
fn deep_24_levels_records_depth_without_stack_overflow() {
    const DEPTH: usize = 24;
    let td = TempDir::new().unwrap();
    let mut p = td.path().to_path_buf();
    for i in 0..DEPTH {
        p.push(format!("level{i:02}"));
        dir(&p);
    }
    write(&p.join("leaf.bin"), 10);
    let tree = scan_path(td.path());

    assert_eq!(tree.root_node().size, 10);
    let leaf = find(&tree, &["level00"]);
    assert_eq!(leaf.depth, 1);
    let names: Vec<String> = (0..DEPTH).map(|i| format!("level{i:02}")).collect();
    let mut segs: Vec<&str> = names.iter().map(String::as_str).collect();
    segs.push("leaf.bin");
    let file = find(&tree, &segs);
    assert_eq!(file.depth, DEPTH as u16 + 1);
    assert_eq!(file.kind, NodeKind::File);
    assert_eq!(file.size, 10);
    assert_eq!(tree.nodes.iter().map(|n| n.depth).max(), Some(DEPTH as u16 + 1));
    assert_eq!(tree.root_node().dir_count, DEPTH as u32);
    assert_eq!(tree.root_node().file_count, 1);
    tree.validate().unwrap();
}

#[cfg(unix)]
#[test]
fn cross_filesystem_scan_skips_other_devices_by_default() {
    use std::os::unix::fs::MetadataExt;
    let shm = Path::new("/dev/shm");
    let (Ok(dev_meta), Ok(shm_meta)) = (fs::metadata("/dev"), fs::metadata(shm)) else {
        eprintln!("skip: /dev/shm 不存在");
        return;
    };
    if dev_meta.dev() == shm_meta.dev() {
        eprintln!("skip: /dev/shm 与 /dev 同设备");
        return;
    }
    if fs::read_dir("/dev").is_err() {
        eprintln!("skip: /dev 不可读");
        return;
    }
    let no_cross = scan(
        Path::new("/dev"),
        &ScanOptions {
            cross_filesystems: false,
            ..Default::default()
        },
    );
    let Ok(no_cross) = no_cross else {
        eprintln!("skip: 扫 /dev 失败");
        return;
    };
    assert!(
        !all_names(&no_cross).iter().any(|n| n == "shm"),
        "cross_filesystems=false 不应进入 /dev/shm"
    );

    let cross = scan(
        Path::new("/dev"),
        &ScanOptions {
            cross_filesystems: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        all_names(&cross).iter().any(|n| n == "shm"),
        "cross_filesystems=true 应包含 /dev/shm"
    );
}

#[test]
fn cancelled_scan_returns_partial_tree_with_valid_invariants() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    for i in 0..4200 {
        write(&root.join(format!("f{i:05}.bin")), 0);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    let tree = scan_with(root, &ScanOptions::default(), Some(cancel), move |p| {
        if p.entries >= 1024 {
            trigger.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();

    assert!(tree.stats.cancelled);
    assert!(tree.stats.elapsed_ms < 1000, "取消后应快速返回");
    assert_eq!(tree.nodes.len(), 4097); // root + 4096 条
    assert_eq!(tree.root_node().file_count, 4096);
    tree.validate().unwrap();
}

#[test]
fn root_path_missing_returns_not_found() {
    let td = TempDir::new().unwrap();
    let err = scan(&td.path().join("nope"), &ScanOptions::default()).unwrap_err();
    assert!(matches!(err, ScanError::NotFound(_)), "err={err:?}");
}

#[test]
fn root_path_is_file_returns_not_a_directory() {
    let td = TempDir::new().unwrap();
    write(&td.path().join("a.bin"), 1);
    let err = scan(&td.path().join("a.bin"), &ScanOptions::default()).unwrap_err();
    assert!(matches!(err, ScanError::NotADirectory(_)), "err={err:?}");
}

#[test]
fn empty_directory_has_zero_size_and_valid_invariants() {
    let td = TempDir::new().unwrap();
    let tree = scan_path(td.path());
    assert_eq!(tree.root_node().size, 0);
    assert_eq!(tree.root_node().file_count, 0);
    assert_eq!(tree.root_node().dir_count, 0);
    assert_eq!(tree.stats.entries, 1);
    assert_eq!(tree.stats.errors, 0);
    tree.validate().unwrap();
}

#[test]
fn directories_without_files_aggregate_zero_but_count_dirs() {
    let td = TempDir::new().unwrap();
    dir(&td.path().join("x/y/z"));
    let tree = scan_path(td.path());
    assert_eq!(tree.root_node().size, 0);
    assert_eq!(tree.root_node().file_count, 0);
    assert_eq!(tree.root_node().dir_count, 3);
    assert_eq!(find(&tree, &["x", "y", "z"]).depth, 3);
    tree.validate().unwrap();
}

#[test]
fn exclude_prefix_skips_subtree() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("keep/a.bin"), 10);
    write(&root.join("skip/b.bin"), 20);
    let tree = scan(
        root,
        &ScanOptions {
            exclude: vec![root.join("skip")],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(tree.root_node().size, 10);
    assert!(!all_names(&tree).iter().any(|n| n == "skip"));
    tree.validate().unwrap();
}

#[test]
fn path_with_space_hash_percent_and_chinese_roundtrips_via_node_path() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    let d = "a b#c%中文";
    let f = "d e#1%.bin";
    write(&root.join(d).join(f), 5);
    let tree = scan_path(root);
    let id = find_id(&tree, &[d, f]);
    assert_eq!(disklens_core::node_path(&tree, id), root.join(d).join(f));
    assert_eq!(find(&tree, &[d]).size, 5);
    tree.validate().unwrap();
}

#[test]
fn mtime_is_recorded_as_unix_seconds() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("a.bin"), 1);
    let tree = scan_path(root);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mtime = find(&tree, &["a.bin"]).mtime;
    assert!((now - mtime).abs() <= 60, "mtime={mtime} now={now}");
}

#[cfg(unix)]
#[test]
fn tree_size_matches_du_within_half_percent_on_fixture() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    for d in ["a", "b", "c"] {
        for i in 0..3 {
            write(&root.join(d).join(format!("f{i}.bin")), 1024 * 1024);
        }
    }
    let tree = scan_path(root);
    let out = std::process::Command::new("du")
        .arg("-sB1")
        .arg(root)
        .output()
        .unwrap();
    assert!(out.status.success());
    let du: u64 = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let diff = (tree.root_node().size as f64 - du as f64).abs() / du as f64;
    assert!(
        diff <= 0.005,
        "tree={} du={du} diff={diff:.4}",
        tree.root_node().size
    );
}
