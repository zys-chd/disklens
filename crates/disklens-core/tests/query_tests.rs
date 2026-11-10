mod support;

use std::path::PathBuf;

use disklens_core::model::{ExtStat, Node, ScanStats};
use disklens_core::{
    aggregate, children, node_path, top_files, type_stats, ChildQuery, NodeId, NodeKind, SortBy,
    Tree, MAX_LIMIT,
};
use support::*;
use tempfile::TempDir;

fn file_node(name: &str, parent: NodeId, size: u64, mtime: i64) -> Node {
    Node {
        name: name.into(),
        parent,
        children: Vec::new(),
        size,
        own_size: size,
        file_count: 0,
        dir_count: 0,
        mtime,
        kind: NodeKind::File,
        ext_id: 0,
        depth: 1,
        error_count: 0,
    }
}

fn dir_node(name: &str, parent: NodeId, file_count: u32, mtime: i64) -> Node {
    Node {
        name: name.into(),
        parent,
        children: Vec::new(),
        size: 0,
        own_size: 0,
        file_count,
        dir_count: 0,
        mtime,
        kind: NodeKind::Dir,
        ext_id: 0,
        depth: 1,
        error_count: 0,
    }
}

fn synthetic(mut children_nodes: Vec<Node>) -> Tree {
    let mut nodes = vec![dir_node("root", disklens_core::NO_PARENT, 0, 0)];
    for (i, n) in children_nodes.iter_mut().enumerate() {
        n.parent = 0;
        nodes.push(n.clone());
        nodes[0].children.push((i + 1) as NodeId);
    }
    let mut tree = Tree {
        nodes,
        root: 0,
        root_path: PathBuf::from("/synthetic"),
        exts: vec!["".into()],
        ext_stats: vec![ExtStat::default()],
        stats: ScanStats::default(),
        safety: Vec::new(),
    };
    aggregate(&mut tree.nodes);
    tree
}

fn q(sort_by: SortBy, desc: bool) -> ChildQuery {
    ChildQuery {
        sort_by,
        desc,
        ..Default::default()
    }
}

fn child_names(tree: &Tree, q: &ChildQuery) -> Vec<String> {
    children(tree, tree.root, q)
        .items
        .iter()
        .map(|&id| tree.node(id).name.to_string())
        .collect()
}

#[test]
fn sorts_by_size_name_files_and_mtime_in_both_directions() {
    let tree = synthetic(vec![
        file_node("b", 0, 300, 30),
        file_node("a", 0, 100, 10),
        file_node("c", 0, 200, 20),
    ]);
    assert_eq!(child_names(&tree, &q(SortBy::Size, true)), ["b", "c", "a"]);
    assert_eq!(child_names(&tree, &q(SortBy::Size, false)), ["a", "c", "b"]);
    assert_eq!(child_names(&tree, &q(SortBy::Name, false)), ["a", "b", "c"]);
    assert_eq!(child_names(&tree, &q(SortBy::Name, true)), ["c", "b", "a"]);
    assert_eq!(
        child_names(&tree, &q(SortBy::Mtime, false)),
        ["a", "c", "b"]
    );
    assert_eq!(child_names(&tree, &q(SortBy::Mtime, true)), ["b", "c", "a"]);

    let by_files = synthetic(vec![
        dir_node("few", 0, 2, 0),
        dir_node("most", 0, 9, 0),
        dir_node("mid", 0, 5, 0),
    ]);
    assert_eq!(
        child_names(&by_files, &q(SortBy::Files, true)),
        ["most", "mid", "few"]
    );
}

#[test]
fn mtime_sort_with_equal_timestamps_is_deterministic() {
    let tree = synthetic(vec![
        file_node("c", 0, 1, 100),
        file_node("a", 0, 1, 100),
        file_node("b", 0, 1, 100),
    ]);
    let query = q(SortBy::Mtime, true);
    let first = child_names(&tree, &query);
    let second = child_names(&tree, &query);
    assert_eq!(first, second);
    assert_eq!(first, ["c", "b", "a"]); // 同 mtime 时按名字确定性排序
}

#[test]
fn filter_matches_name_substring_case_insensitively() {
    let tree = synthetic(vec![
        file_node("Photo.JPG", 0, 3, 0),
        file_node("photo-backup.jpg", 0, 2, 0),
        file_node("movie.mp4", 0, 1, 0),
    ]);
    let mut query = q(SortBy::Size, true);
    query.filter = Some("PHOTO".into());
    let page = children(&tree, tree.root, &query);
    assert_eq!(page.total, 2);
    assert_eq!(
        child_names(&tree, &query),
        ["Photo.JPG", "photo-backup.jpg"]
    );
}

#[test]
fn min_size_filters_before_pagination() {
    let tree = synthetic(vec![
        file_node("big", 0, 1000, 0),
        file_node("mid", 0, 500, 0),
        file_node("small", 0, 10, 0),
    ]);
    let mut query = q(SortBy::Size, true);
    query.min_size = Some(500);
    query.limit = 1;
    let page = children(&tree, tree.root, &query);
    assert_eq!(page.total, 2);
    assert_eq!(page.items.len(), 1);
    assert_eq!(tree.node(page.items[0]).name.as_ref(), "big");

    query.offset = 1;
    let page = children(&tree, tree.root, &query);
    assert_eq!(tree.node(page.items[0]).name.as_ref(), "mid");
}

#[test]
fn offset_beyond_total_returns_empty_page_not_error() {
    let tree = synthetic(vec![file_node("a", 0, 1, 0)]);
    let mut query = q(SortBy::Size, true);
    query.offset = 99;
    let page = children(&tree, tree.root, &query);
    assert_eq!(page.total, 1);
    assert!(page.items.is_empty());
}

#[test]
fn limit_is_capped_at_5000() {
    let nodes: Vec<Node> = (0..6000)
        .map(|i| file_node(&format!("f{i:05}"), 0, i as u64, 0))
        .collect();
    let tree = synthetic(nodes);
    assert_eq!(MAX_LIMIT, 5000);
    let mut query = q(SortBy::Size, true);
    query.limit = 999_999;
    let page = children(&tree, tree.root, &query);
    assert_eq!(page.total, 6000);
    assert_eq!(page.items.len(), 5000);
    assert_eq!(tree.node(page.items[0]).size, 5999);
}

#[test]
fn list_children_paginates_5000_files_without_overlap() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    for i in 0..5000 {
        write(&root.join(format!("f{i:04}.txt")), 0);
    }
    let tree = scan_path(root);
    assert_eq!(tree.root_node().file_count, 5000);

    let mut seen = std::collections::HashSet::new();
    let mut total = 0usize;
    for page in 0..50 {
        let query = ChildQuery {
            sort_by: SortBy::Name,
            desc: false,
            offset: page * 100,
            limit: 100,
            ..Default::default()
        };
        let res = children(&tree, tree.root, &query);
        assert_eq!(res.total, 5000);
        assert_eq!(res.items.len(), 100, "page {page}");
        total += res.items.len();
        for id in res.items {
            assert!(seen.insert(id), "重复节点 {id}");
        }
    }
    assert_eq!(total, 5000);
    assert_eq!(seen.len(), 5000);

    let query = ChildQuery {
        offset: 5000,
        ..Default::default()
    };
    let res = children(&tree, tree.root, &query);
    assert_eq!(res.total, 5000);
    assert!(res.items.is_empty());

    let query = ChildQuery {
        offset: 9999,
        ..Default::default()
    };
    assert!(children(&tree, tree.root, &query).items.is_empty());
}

#[test]
fn node_path_rebuilds_absolute_path_from_names() {
    let td = TempDir::new().unwrap();
    // 期望值必须用规范化的根：Windows 上 TempDir 给的是 8.3 短名（RUNNER~1），
    // 而扫描器存的是规范化后的长名（\\?\C:\Users\runneradmin\...）。两者都不是错的，只是口径不同。
    let root = std::fs::canonicalize(td.path()).unwrap();
    write(&root.join("a b").join("中文#1%").join("x.bin"), 9);
    let tree = scan_path(&root);
    let id = support::find_id(&tree, &["a b", "中文#1%", "x.bin"]);
    let expect = root.join("a b").join("中文#1%").join("x.bin");
    assert_eq!(node_path(&tree, id), expect);
    assert_eq!(node_path(&tree, tree.root), root);
}

#[test]
fn top_files_returns_largest_k_in_descending_order() {
    let td = TempDir::new().unwrap();
    let root = std::fs::canonicalize(td.path()).unwrap();
    write(&root.join("a").join("one.bin"), 100);
    write(&root.join("a").join("two.bin"), 5000);
    write(&root.join("b").join("three.bin"), 300);
    let tree = scan_path(&root);

    let top = top_files(&tree, 2, None);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].name, "two.bin");
    assert_eq!(top[0].size, 5000);
    assert_eq!(top[0].path, root.join("a").join("two.bin"));
    assert_eq!(top[1].name, "three.bin");
    assert_eq!(top[1].size, 300);

    let top = top_files(&tree, 10, Some(1000));
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].name, "two.bin");
    assert!(top_files(&tree, 0, None).is_empty());
}

#[test]
fn type_stats_groups_extensions_case_insensitively_and_interns() {
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("a.JPG"), 100);
    write(&root.join("b.jpg"), 200);
    write(&root.join("c.mp4"), 1000);
    write(&root.join("noext"), 50);
    write(&root.join("archive.tar.gz"), 300);
    write(&root.join("readme"), 0);
    let tree = scan_path(root);

    let stats = type_stats(&tree, 10);
    assert_eq!(stats.len(), 4);
    assert_eq!(stats[0].ext, "mp4");
    assert_eq!(stats[0].bytes, 1000);
    assert_eq!(stats[0].files, 1);
    assert_eq!(stats[1].ext, "gz");
    assert_eq!(stats[1].bytes, 300);
    assert_eq!(stats[2].ext, "jpg");
    assert_eq!(stats[2].bytes, 300);
    assert_eq!(stats[2].files, 2);
    assert_eq!(stats[3].ext, "");
    assert_eq!(stats[3].bytes, 50);
    assert_eq!(stats[3].files, 2); // noext + readme

    // 只有 4 个不同扩展名（含空串），intern 表不应有重复
    assert_eq!(tree.exts.len(), 4);
    let top2 = type_stats(&tree, 2);
    assert_eq!(top2.len(), 2);
    assert_eq!(top2[0].ext, "mp4");
    assert_eq!(top2[1].ext, "gz");
}

#[test]
fn children_of_file_node_is_empty_page() {
    let tree = synthetic(vec![file_node("a", 0, 1, 0)]);
    let page = children(&tree, 1, &ChildQuery::default());
    assert_eq!(page.total, 0);
    assert!(page.items.is_empty());
}
