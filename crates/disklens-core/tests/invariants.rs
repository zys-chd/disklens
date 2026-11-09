mod support;

use disklens_core::{NodeKind, NO_PARENT};
use support::*;
use tempfile::TempDir;

fn fixture() -> (TempDir, disklens_core::Tree) {
    let td = TempDir::new().unwrap();
    let root = td.path();
    write(&root.join("a/b/c/deep.bin"), 111);
    write(&root.join("a/b/side.bin"), 222);
    write(&root.join("top.bin"), 333);
    dir(&root.join("empty"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink("a/b", root.join("link")).unwrap();
    }
    let tree = scan_path(root);
    (td, tree)
}

#[test]
fn parent_chain_of_every_node_terminates_at_root_without_cycles() {
    let (_td, tree) = fixture();
    let mut checked = 0usize;
    for (idx, node) in tree.nodes.iter().enumerate() {
        let mut cur = idx as u32;
        let mut steps = 0usize;
        loop {
            assert!(steps <= tree.nodes.len(), "节点 {idx} 的 parent 链成环");
            if cur == tree.root {
                break;
            }
            let n = tree.node(cur);
            assert_ne!(n.parent, NO_PARENT, "节点 {cur} 的 parent 链断了");
            assert!(
                n.parent < cur,
                "child id 必须大于 parent：{cur} <- {}",
                n.parent
            );
            cur = n.parent;
            steps += 1;
        }
        if node.kind == NodeKind::Dir {
            assert!(node.size >= node.own_size, "目录 size 至少等于 own_size");
        } else {
            assert!(node.children.is_empty(), "非目录不应有 children");
        }
        checked += 1;
    }
    assert_eq!(checked, tree.nodes.len());
    assert_eq!(tree.root_node().parent, NO_PARENT);
}

#[test]
fn ids_are_unique_and_children_parent_links_are_consistent() {
    let (_td, tree) = fixture();
    let mut seen = std::collections::HashSet::new();
    for (idx, node) in tree.nodes.iter().enumerate() {
        assert!(seen.insert(idx), "重复 id {idx}");
        for &c in &node.children {
            assert_eq!(tree.node(c).parent, idx as u32);
        }
    }
    assert_eq!(seen.len(), tree.nodes.len());
    tree.validate().unwrap();
}

#[test]
fn validate_catches_corrupted_size_and_parent_loop() {
    let (_td, tree) = fixture();
    let mut broken = tree.clone();
    broken.nodes[0].size += 1;
    let err = broken.validate().unwrap_err();
    assert!(err.contains("I1 失败"), "{err}");

    let mut broken = tree.clone();
    broken.nodes[1].parent = 1;
    let err = broken.validate().unwrap_err();
    assert!(err.contains("不小于自身"), "{err}");
}
