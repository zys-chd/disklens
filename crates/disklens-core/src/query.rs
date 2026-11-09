use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{NodeId, NodeKind, Tree, NO_PARENT};

pub const MAX_LIMIT: usize = 5000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    #[default]
    Size,
    Name,
    Files,
    Mtime,
}

#[derive(Clone, Debug)]
pub struct ChildQuery {
    pub sort_by: SortBy,
    pub desc: bool,
    pub offset: usize,
    pub limit: usize,
    pub min_size: Option<u64>,
    pub filter: Option<String>,
}

impl Default for ChildQuery {
    fn default() -> Self {
        Self {
            sort_by: SortBy::Size,
            desc: true,
            offset: 0,
            limit: 200,
            min_size: None,
            filter: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChildPage {
    /// 过滤后、分页前的总数
    pub total: usize,
    pub items: Vec<NodeId>,
}

/// 子节点分页。filter 为文件名子串（不区分大小写）；offset 越界返回空。
pub fn children(tree: &Tree, node: NodeId, q: &ChildQuery) -> ChildPage {
    let Some(n) = tree.nodes.get(node as usize) else {
        return ChildPage {
            total: 0,
            items: Vec::new(),
        };
    };
    let needle = q.filter.as_ref().map(|f| f.to_lowercase());
    let mut items: Vec<NodeId> = n
        .children
        .iter()
        .copied()
        .filter(|&id| {
            let c = tree.node(id);
            q.min_size.is_none_or(|m| c.size >= m)
                && needle
                    .as_ref()
                    .is_none_or(|f| c.name.to_lowercase().contains(f))
        })
        .collect();
    let total = items.len();
    items.sort_by(|&a, &b| {
        let (ca, cb) = (tree.node(a), tree.node(b));
        let ord = match q.sort_by {
            SortBy::Size => ca.size.cmp(&cb.size),
            SortBy::Files => ca.file_count.cmp(&cb.file_count),
            SortBy::Mtime => ca.mtime.cmp(&cb.mtime),
            SortBy::Name => ca.name.to_lowercase().cmp(&cb.name.to_lowercase()),
        }
        .then_with(|| ca.name.cmp(&cb.name))
        .then_with(|| a.cmp(&b));
        if q.desc {
            ord.reverse()
        } else {
            ord
        }
    });
    let limit = q.limit.min(MAX_LIMIT);
    let items = items
        .into_iter()
        .skip(q.offset)
        .take(limit)
        .collect::<Vec<_>>();
    ChildPage { total, items }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopFile {
    pub id: NodeId,
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub mtime: i64,
}

/// 单趟线性扫描 + 大小 k 的小顶堆（Reverse 包一层）。
pub fn top_files(tree: &Tree, limit: usize, min_size: Option<u64>) -> Vec<TopFile> {
    if limit == 0 {
        return Vec::new();
    }
    let mut heap: BinaryHeap<Reverse<(u64, NodeId)>> = BinaryHeap::with_capacity(limit.min(1024));
    for (id, n) in tree.nodes.iter().enumerate() {
        if n.kind != NodeKind::File {
            continue;
        }
        if min_size.is_some_and(|m| n.size < m) {
            continue;
        }
        let id = id as NodeId;
        if heap.len() < limit {
            heap.push(Reverse((n.size, id)));
        } else if let Some(&Reverse((min, _))) = heap.peek() {
            if n.size > min {
                heap.pop();
                heap.push(Reverse((n.size, id)));
            }
        }
    }
    let mut ids: Vec<(u64, NodeId)> = heap.into_iter().map(|Reverse(x)| x).collect();
    ids.sort_unstable_by(|a, b| b.cmp(a));
    ids.into_iter()
        .map(|(size, id)| {
            let n = tree.node(id);
            TopFile {
                id,
                name: n.name.to_string(),
                path: node_path(tree, id),
                size,
                mtime: n.mtime,
            }
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeStat {
    pub ext: String,
    pub bytes: u64,
    pub files: u32,
}

/// 按扩展名聚合（扩展名在扫描时已 intern，无扩展名归 ""）。
pub fn type_stats(tree: &Tree, top_n: usize) -> Vec<TypeStat> {
    let mut out: Vec<TypeStat> = tree
        .exts
        .iter()
        .enumerate()
        .filter_map(|(i, ext)| {
            let s = tree.ext_stats.get(i)?;
            (s.files > 0).then(|| TypeStat {
                ext: ext.to_string(),
                bytes: s.bytes,
                files: s.files,
            })
        })
        .collect();
    out.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.ext.cmp(&b.ext)));
    out.truncate(top_n);
    out
}

/// 绝对路径按需从祖先链拼出（节点只存文件名）。
pub fn node_path(tree: &Tree, id: NodeId) -> PathBuf {
    let mut parts: Vec<&str> = Vec::new();
    let mut cur = id;
    while cur != NO_PARENT {
        let Some(n) = tree.nodes.get(cur as usize) else {
            break;
        };
        if cur == tree.root {
            break;
        }
        parts.push(&n.name);
        cur = n.parent;
    }
    let mut path = tree.root_path.clone();
    for part in parts.iter().rev() {
        path.push(part);
    }
    path
}
