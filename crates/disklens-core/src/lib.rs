//! DiskLens 核心：扫描 / 聚合 / 查询 / 快照 / 护栏 / 文件操作。纯 Rust，无 Tauri 依赖。

pub mod aggregate;
pub mod guard;
pub mod model;
pub mod ops;
pub mod path;
pub mod query;
pub mod rules;
pub mod scan;
pub mod snapshot;

pub use aggregate::{aggregate, aggregate_tree};
pub use guard::is_protected;
pub use model::{ExtStat, Node, NodeId, NodeKind, ScanStats, Tree, NO_PARENT};
pub use ops::{delete_permanent, move_to_trash, OpFailure, OpResult};
pub use path::norm_path_str;
pub use query::{
    children, node_path, top_files, type_stats, ChildPage, ChildQuery, SortBy, TopFile, TypeStat,
    MAX_LIMIT,
};
pub use rules::{
    classify_tree, default_rules, default_rules_path, safety_stats, Rule, RuleSet, Safety,
    SafetyLevel, SafetyReport, SafetyStat, NO_RULE,
};
pub use scan::{scan, scan_with, Progress, ScanError, ScanOptions};
pub use snapshot::{
    default_snapshot_dir, diff_snapshots, list_snapshots, load_snapshot, save_snapshot,
    save_snapshot_to, Diff, DiffRow, SnapError, Snapshot, SnapshotMeta,
};
