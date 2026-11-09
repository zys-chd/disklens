mod support;

use std::fs;

use disklens_core::{diff_snapshots, list_snapshots, load_snapshot, save_snapshot_to, Snapshot};
use support::*;
use tempfile::TempDir;

const MIB: u64 = 1024 * 1024;

fn snap(created: i64, files: Vec<(&str, u64)>, dirs: Vec<(&str, u64)>) -> Snapshot {
    let files: Vec<(String, u64)> = files.into_iter().map(|(p, s)| (p.into(), s)).collect();
    let dirs: Vec<(String, u64)> = dirs.into_iter().map(|(p, s)| (p.into(), s)).collect();
    let total =
        files.iter().map(|(_, s)| *s).sum::<u64>() + dirs.iter().map(|(_, s)| *s).sum::<u64>();
    Snapshot {
        v: 1,
        root_path: "/root".into(),
        created_at: created,
        total_bytes: total,
        dirs,
        files,
    }
}

#[test]
fn snapshot_roundtrip_preserves_totals_dirs_and_top_files() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("bigdir/big.bin"), 2 * 1024 * 1024);
    write(&root.join("small/small.bin"), 1024);
    let tree = scan_path(root);

    let dir = td.path().join("snapshots");
    let path = save_snapshot_to(&tree, &dir).unwrap();
    assert!(path.exists());
    assert_eq!(path.extension().unwrap(), "gz");
    let magic = fs::read(&path).unwrap();
    assert_eq!(&magic[..2], &[0x1f, 0x8b], "必须是 gzip");

    let loaded = load_snapshot(&path).unwrap();
    assert_eq!(loaded.v, 1);
    assert_eq!(loaded.root_path, root.to_string_lossy());
    assert_eq!(loaded.total_bytes, tree.root_node().size);
    assert_eq!(loaded.files.len(), 2);
    assert_eq!(loaded.files[0].1, 2 * 1024 * 1024);
    assert_eq!(
        loaded.files[0].0,
        root.join("bigdir/big.bin").to_string_lossy().into_owned()
    );
    let root_s = root.to_string_lossy().into_owned();
    let bigdir_s = root.join("bigdir").to_string_lossy().into_owned();
    let small_s = root.join("small").to_string_lossy().into_owned();
    let dir_paths: Vec<&str> = loaded.dirs.iter().map(|(p, _)| p.as_str()).collect();
    assert!(dir_paths.contains(&root_s.as_str()));
    assert!(dir_paths.contains(&bigdir_s.as_str()));
    assert!(
        !dir_paths.contains(&small_s.as_str()),
        "小于 1MiB 的目录不进快照"
    );

    let metas = list_snapshots(&dir).unwrap();
    assert_eq!(metas.len(), 1);
    assert_eq!(metas[0].total_bytes, tree.root_node().size);
    assert_eq!(metas[0].root_path, root.to_string_lossy());
    assert_eq!(metas[0].entries, loaded.dirs.len() + loaded.files.len());
}

#[test]
fn snapshots_include_root_path_and_only_big_dirs() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("big/a.bin"), 1 << 20);
    write(&root.join("tiny/a.bin"), 10);
    let tree = scan_path(root);
    let dir = td.path().join("snaps");
    let path = save_snapshot_to(&tree, &dir).unwrap();
    let loaded = load_snapshot(&path).unwrap();
    let root_s = root.to_string_lossy().into_owned();
    let big_s = root.join("big").to_string_lossy().into_owned();
    let tiny_s = root.join("tiny").to_string_lossy().into_owned();
    assert!(loaded.dirs.iter().any(|(p, _)| p == &root_s));
    assert!(loaded.dirs.iter().any(|(p, _)| p == &big_s));
    assert!(!loaded.dirs.iter().any(|(p, _)| p == &tiny_s));
}

#[test]
fn diff_reports_growth_shrink_add_delete_and_ignores_sub_mib() {
    let a = snap(
        1,
        vec![
            ("/b-grow", 10 * MIB),
            ("/shrink", 10 * MIB),
            ("/deleted", 5 * MIB),
            ("/same", 5 * MIB),
            ("/tiny", 5 * MIB),
        ],
        vec![],
    );
    let b = snap(
        2,
        vec![
            ("/b-grow", 13 * MIB),
            ("/shrink", 7 * MIB),
            ("/same", 5 * MIB),
            ("/tiny", 5 * MIB + MIB / 2),
            ("/a-added", 3 * MIB),
        ],
        vec![],
    );
    let d = diff_snapshots(&a, &b, 10);
    assert_eq!(d.grown.len(), 2);
    assert_eq!(d.grown[0].path, "/a-added");
    assert_eq!(d.grown[0].delta, 3 * MIB as i64);
    assert_eq!(d.grown[1].path, "/b-grow");
    assert_eq!(d.grown[1].delta, 3 * MIB as i64);
    assert_eq!(d.grown_bytes, 6 * MIB);

    assert_eq!(d.shrunk.len(), 2);
    assert_eq!(d.shrunk[0].path, "/deleted");
    assert_eq!(d.shrunk[0].delta, -(5 * MIB as i64));
    assert_eq!(d.shrunk[0].size_a, 5 * MIB);
    assert_eq!(d.shrunk[0].size_b, 0);
    assert_eq!(d.shrunk[1].path, "/shrink");
    assert_eq!(d.shrunk[1].delta, -(3 * MIB as i64));
    assert_eq!(d.shrunk_bytes, 8 * MIB);
    assert!(!d.grown.iter().any(|r| r.path == "/tiny"));
    assert!(!d.shrunk.iter().any(|r| r.path == "/tiny"));
    assert!(!d.grown.iter().any(|r| r.path == "/same"));
    assert!(!d.shrunk.iter().any(|r| r.path == "/same"));
}

#[test]
fn diff_limit_truncates_rows_but_keeps_full_byte_totals() {
    let a = snap(1, vec![("/a", 0), ("/b", 0), ("/c", 0)], vec![]);
    let b = snap(
        2,
        vec![("/a", 2 * MIB), ("/b", 3 * MIB), ("/c", 4 * MIB)],
        vec![],
    );
    let d = diff_snapshots(&a, &b, 1);
    assert_eq!(d.grown.len(), 1);
    assert_eq!(d.grown[0].path, "/c");
    assert_eq!(d.grown[0].delta, 4 * MIB as i64);
    assert_eq!(d.grown_bytes, 9 * MIB);
}

#[test]
fn diff_handles_u64_extremes_without_overflow_or_panic() {
    let big_a = snap(1, vec![("/max", u64::MAX)], vec![]);
    let zero_b = snap(2, vec![], vec![]);
    let d = diff_snapshots(&big_a, &zero_b, 10);
    assert_eq!(d.shrunk.len(), 1);
    assert_eq!(d.shrunk[0].delta, -i64::MAX);
    assert_eq!(d.shrunk_bytes, i64::MAX as u64);

    let zero_a = snap(1, vec![], vec![]);
    let big_b = snap(2, vec![("/max", u64::MAX)], vec![]);
    let d = diff_snapshots(&zero_a, &big_b, 10);
    assert_eq!(d.grown.len(), 1);
    assert_eq!(d.grown[0].delta, i64::MAX);
    assert_eq!(d.grown_bytes, i64::MAX as u64);
}

#[test]
fn diff_joins_directory_sizes_too() {
    let a = snap(1, vec![], vec![("/dir", 10 * MIB), ("/gone", 2 * MIB)]);
    let b = snap(2, vec![], vec![("/dir", 12 * MIB)]);
    let d = diff_snapshots(&a, &b, 10);
    assert_eq!(d.grown.len(), 1);
    assert_eq!(d.grown[0].path, "/dir");
    assert_eq!(d.grown[0].delta, 2 * MIB as i64);
    assert_eq!(d.shrunk.len(), 1);
    assert_eq!(d.shrunk[0].path, "/gone");
    assert_eq!(d.shrunk[0].delta, -(2 * MIB as i64));
}

#[test]
fn list_snapshots_missing_dir_returns_empty_list() {
    let td = TempDir::new().unwrap();
    let metas = list_snapshots(&td.path().join("nope")).unwrap();
    assert!(metas.is_empty());
}

#[test]
fn list_snapshots_sorts_newest_first() {
    let td = TempDir::new().unwrap();
    let dir = td.path().join("snaps");
    let a = snap(100, vec![("/a", 1)], vec![]);
    let b = snap(200, vec![("/b", 1)], vec![]);
    for s in [&a, &b] {
        let path = dir.join(format!("s-{}.json.gz", s.created_at));
        fs::create_dir_all(&dir).unwrap();
        let f = fs::File::create(&path).unwrap();
        let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        serde_json::to_writer(&mut enc, s).unwrap();
        enc.finish().unwrap();
    }
    let metas = list_snapshots(&dir).unwrap();
    assert_eq!(metas.len(), 2);
    assert_eq!(metas[0].created_at, 200);
    assert_eq!(metas[1].created_at, 100);
}

#[test]
fn snapshot_struct_serializes_with_camel_case_keys() {
    let s = snap(7, vec![("/f", 1)], vec![("/d", 2)]);
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"rootPath\":\"/root\""), "{json}");
    assert!(json.contains("\"createdAt\":7"), "{json}");
    assert!(json.contains("\"totalBytes\""), "{json}");
    assert!(json.contains("\"dirs\":[[\"/d\",2]]"), "{json}");
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.files, vec![("/f".to_string(), 1)]);
}
