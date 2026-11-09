#![allow(dead_code)]

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use disklens_core::{scan, Node, NodeId, ScanOptions, Tree};

pub fn write(p: &Path, bytes: usize) -> u64 {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = File::create(p).unwrap();
    f.write_all(&vec![0xAB; bytes]).unwrap();
    bytes as u64
}

pub fn sparse(p: &Path, len: u64) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let f = File::create(p).unwrap();
    f.set_len(len).unwrap();
}

pub fn dir(p: &Path) {
    fs::create_dir_all(p).unwrap();
}

pub fn scan_path(p: &Path) -> Tree {
    scan(p, &ScanOptions::default()).unwrap()
}

/// 按从根到叶的文件名找到节点；找不到直接 panic（测试里不允许弱断言）。
pub fn find<'a>(tree: &'a Tree, segs: &[&str]) -> &'a Node {
    let mut node = tree.root_node();
    for seg in segs {
        let id = node
            .children
            .iter()
            .copied()
            .find(|&c| &*tree.node(c).name == *seg)
            .unwrap_or_else(|| panic!("找不到 {seg:?}，父节点 {:?}", node.name));
        node = tree.node(id);
    }
    node
}

pub fn find_id(tree: &Tree, segs: &[&str]) -> NodeId {
    let mut node = tree.root_node();
    for seg in segs {
        let id = node
            .children
            .iter()
            .copied()
            .find(|&c| &*tree.node(c).name == *seg)
            .unwrap_or_else(|| panic!("找不到 {seg:?}"));
        node = tree.node(id);
    }
    // 返回最后一个节点的 id：重走一次以拿到 id
    let mut cur = tree.root;
    for seg in segs {
        cur = tree
            .node(cur)
            .children
            .iter()
            .copied()
            .find(|&c| &*tree.node(c).name == *seg)
            .unwrap();
    }
    cur
}

pub fn names_at(tree: &Tree, node: NodeId) -> Vec<String> {
    tree.node(node)
        .children
        .iter()
        .map(|&c| tree.node(c).name.to_string())
        .collect()
}

pub fn all_names(tree: &Tree) -> Vec<String> {
    tree.nodes.iter().map(|n| n.name.to_string()).collect()
}

pub fn count_kind(tree: &Tree, kind: disklens_core::NodeKind) -> usize {
    tree.nodes.iter().filter(|n| n.kind == kind).count()
}
