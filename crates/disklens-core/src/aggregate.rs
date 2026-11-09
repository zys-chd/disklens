use crate::model::{Node, NodeKind, Tree, NO_PARENT};

/// 逆序单趟聚合。前序建树保证 child id > parent id。
pub fn aggregate(nodes: &mut [Node]) {
    for i in (0..nodes.len()).rev() {
        let (parent, size, files, dirs, kind) = {
            let node = &mut nodes[i];
            if node.kind == NodeKind::Dir {
                node.size += node.own_size;
            }
            (
                node.parent,
                node.size,
                node.file_count,
                node.dir_count,
                node.kind,
            )
        };
        if parent == NO_PARENT {
            continue;
        }
        let p = &mut nodes[parent as usize];
        match kind {
            NodeKind::Dir => {
                // 目录的 size 已含 own_size（直属文件），只把目录子树并上去
                p.size += size;
                p.file_count += files;
                p.dir_count += dirs + 1;
            }
            NodeKind::File => p.file_count += 1,
            NodeKind::Symlink => {}
        }
    }
}

pub fn aggregate_tree(tree: &mut Tree) {
    aggregate(&mut tree.nodes);
    if let Err(e) = tree.validate() {
        debug_assert!(false, "聚合后不变量失败: {e}");
    }
}
