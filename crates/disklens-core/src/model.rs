use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::rules::Safety;

pub type NodeId = u32;
pub const NO_PARENT: NodeId = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Dir,
    File,
    Symlink,
}

/// 一个扫描条目。`size`/`file_count`/`dir_count` 是聚合后的子树值。
#[derive(Clone, Debug)]
pub struct Node {
    pub name: Box<str>,
    pub parent: NodeId,
    pub children: Vec<NodeId>,
    pub size: u64,
    pub own_size: u64,
    pub file_count: u32,
    pub dir_count: u32,
    pub mtime: i64,
    pub kind: NodeKind,
    /// 扩展名 intern 下标；0 = 无扩展名
    pub ext_id: u16,
    pub depth: u16,
    /// 直属不可读条目数（只算本目录）
    pub error_count: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExtStat {
    pub bytes: u64,
    pub files: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub elapsed_ms: u64,
    pub entries: u64,
    pub errors: u64,
    pub cancelled: bool,
    /// 是否做了硬链接去重。Windows 稳定版 std 读不到 nlink/file_index，恒为 false
    pub hardlink_dedup: bool,
}

#[derive(Clone, Debug)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: NodeId,
    pub root_path: PathBuf,
    pub exts: Vec<Box<str>>,
    pub ext_stats: Vec<ExtStat>,
    pub stats: ScanStats,
    /// 与 nodes 同索引的安全等级；未分类时为空/未知，用 `classify_tree` 填充
    pub safety: Vec<Safety>,
}

impl Tree {
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn root_node(&self) -> &Node {
        self.node(self.root)
    }

    /// 校验 I1 / I2 / I4。测试与 debug 断言用；O(n)。
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.is_empty() {
            return Err("nodes 为空".into());
        }
        if self.root as usize >= self.nodes.len() {
            return Err(format!("root {} 越界", self.root));
        }
        let n = self.nodes.len();
        for (idx, node) in self.nodes.iter().enumerate() {
            let id = idx as NodeId;
            if id == self.root {
                if node.parent != NO_PARENT {
                    return Err(format!("root 的 parent 应为 NO_PARENT: {}", node.parent));
                }
            } else {
                if node.parent == NO_PARENT {
                    return Err(format!("节点 {id} 无 parent"));
                }
                if node.parent >= id {
                    return Err(format!("节点 {id} 的 parent {} 不小于自身", node.parent));
                }
            }
            // I4: parent 链必须到达 root，且不能成环（parent<id 已排环，这里再查边界）
            let mut cur = id;
            let mut steps = 0usize;
            while cur != self.root {
                let p = self.nodes.get(cur as usize).ok_or("parent 越界")?.parent;
                if p == NO_PARENT || p as usize >= n {
                    return Err(format!("节点 {id} 的 parent 链断在 {cur}"));
                }
                cur = p;
                steps += 1;
                if steps > n {
                    return Err(format!("节点 {id} 的 parent 链成环"));
                }
            }
        }
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.kind != NodeKind::Dir {
                continue;
            }
            let mut sum_size = node.own_size;
            let mut sum_files = 0u32;
            let mut sum_dirs = 0u32;
            for &c in &node.children {
                let child = &self.nodes[c as usize];
                if child.parent != idx as NodeId {
                    return Err(format!("节点 {idx} 的子节点 {c} parent 不符"));
                }
                if child.kind == NodeKind::Dir {
                    sum_size += child.size;
                    sum_dirs += child.dir_count + 1;
                }
                sum_files += child.file_count + u32::from(child.kind == NodeKind::File);
            }
            // I1
            if node.size != sum_size {
                return Err(format!(
                    "I1 失败 @{} {:?}: size={} own+children={}",
                    idx, node.name, node.size, sum_size
                ));
            }
            // I2
            if node.file_count != sum_files {
                return Err(format!(
                    "I2 失败 @{} {:?}: file_count={} expected={}",
                    idx, node.name, node.file_count, sum_files
                ));
            }
            if node.dir_count != sum_dirs {
                return Err(format!(
                    "dir_count 失败 @{} {:?}: {} != {}",
                    idx, node.name, node.dir_count, sum_dirs
                ));
            }
        }
        Ok(())
    }
}
