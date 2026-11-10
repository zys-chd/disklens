use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, UNIX_EPOCH};

use crate::aggregate::aggregate_tree;
use crate::model::{ExtStat, Node, NodeId, NodeKind, ScanStats, Tree, NO_PARENT};

const CANCEL_CHECK_EVERY: u64 = 4096;
const PROGRESS_EVERY: u64 = 1024;

#[derive(Clone, Debug, Default)]
pub struct ScanOptions {
    /// true 时进入其它设备上的挂载点；默认 false（与 `du -x` 一致）
    pub cross_filesystems: bool,
    /// v1 只支持不跟随符号链接
    pub follow_symlinks: bool,
    /// 绝对路径前缀；命中则不进入。内置排除 /proc /sys /dev
    pub exclude: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct Progress {
    pub entries: u64,
    pub bytes: u64,
    pub current_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("路径不存在或无法解析: {0}")]
    NotFound(PathBuf),
    #[error("不是目录: {0}")]
    NotADirectory(PathBuf),
    #[error("无法读取根目录 {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("follow_symlinks=true 暂不支持（v1 固定不跟随符号链接）")]
    FollowSymlinksUnsupported,
}

pub fn scan(path: &Path, options: &ScanOptions) -> Result<Tree, ScanError> {
    scan_with(path, options, None, |_| {})
}

/// 取消通过 `cancel` 轮询（每 4096 条检查一次）；取消后返回已扫部分。
pub fn scan_with<F: FnMut(Progress)>(
    path: &Path,
    options: &ScanOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: F,
) -> Result<Tree, ScanError> {
    if options.follow_symlinks {
        return Err(ScanError::FollowSymlinksUnsupported);
    }
    let root_path = fs::canonicalize(path).map_err(|_| ScanError::NotFound(path.to_path_buf()))?;
    let root_meta = fs::symlink_metadata(&root_path).map_err(|source| ScanError::Io {
        path: root_path.clone(),
        source,
    })?;
    if !root_meta.is_dir() {
        return Err(ScanError::NotADirectory(root_path));
    }

    let root_dev = device_of(&root_meta);
    let mut b = Builder {
        nodes: Vec::new(),
        root_path: root_path.clone(),
        exclude_keys: options
            .exclude
            .iter()
            .map(|p| {
                let canon = fs::canonicalize(p).unwrap_or_else(|_| p.clone());
                crate::path::norm_path_str(&canon)
            })
            .collect(),
        exts: vec![Box::from("")],
        ext_index: HashMap::new(),
        ext_stats: Vec::new(),
        seen_links: HashMap::new(),
        options,
        cancel,
        progress,
        root_dev,
        entries: 0,
        bytes: 0,
        cancelled: false,
        started: Instant::now(),
    };
    b.ext_stats.push(ExtStat::default());
    b.push_node(Node {
        name: root_path
            .file_name()
            .map(|n| n.to_string_lossy().into())
            .unwrap_or_else(|| root_path.to_string_lossy().into()),
        parent: NO_PARENT,
        children: Vec::new(),
        size: 0,
        own_size: 0,
        file_count: 0,
        dir_count: 0,
        mtime: mtime_of(&root_meta),
        kind: NodeKind::Dir,
        ext_id: 0,
        depth: 0,
        error_count: 0,
    });
    b.walk_dir(&root_path, 0, 0);

    let elapsed_ms = b.started.elapsed().as_millis() as u64;
    let entries = b.nodes.len() as u64;
    let errors = b.nodes.iter().map(|n| u64::from(n.error_count)).sum();
    let mut tree = Tree {
        nodes: b.nodes,
        root: 0,
        root_path,
        exts: b.exts,
        ext_stats: b.ext_stats,
        stats: ScanStats {
            elapsed_ms,
            entries,
            errors,
            cancelled: b.cancelled,
            hardlink_dedup: HARDLINK_DEDUP,
        },
        safety: Vec::new(),
    };
    aggregate_tree(&mut tree);
    Ok(tree)
}

struct Builder<'a, F: FnMut(Progress)> {
    nodes: Vec<Node>,
    root_path: PathBuf,
    /// 归一化为 `/` 分隔的规范化字符串，扫描时只需与归一化后的路径比前缀
    /// （否则调用方给的 `RUNNER~1` 短名 / 符号链接路径与 canonicalize 后的根对不上，排除会静默失效）
    exclude_keys: Vec<String>,
    exts: Vec<Box<str>>,
    ext_index: HashMap<Box<str>, u16>,
    ext_stats: Vec<ExtStat>,
    seen_links: HashMap<(u64, u64), ()>,
    options: &'a ScanOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: F,
    root_dev: Option<u64>,
    entries: u64,
    bytes: u64,
    cancelled: bool,
    started: Instant,
}

impl<F: FnMut(Progress)> Builder<'_, F> {
    fn push_node(&mut self, node: Node) -> NodeId {
        let id = self.nodes.len() as NodeId;
        if node.parent != NO_PARENT {
            self.nodes[node.parent as usize].children.push(id);
        }
        self.nodes.push(node);
        id
    }

    fn error(&mut self, parent: NodeId) {
        self.nodes[parent as usize].error_count += 1;
    }

    fn is_excluded(&self, path: &Path) -> bool {
        const BUILTIN: &[&str] = &["/proc", "/sys", "/dev"];
        // 直接扫 /proc 或 /dev 时不能把根自己排除掉
        if !BUILTIN.iter().any(|p| self.root_path.starts_with(p))
            && BUILTIN.iter().any(|p| path.starts_with(p))
        {
            return true;
        }
        if self.exclude_keys.is_empty() {
            return false;
        }
        let cand = crate::path::norm_path_str(path);
        self.exclude_keys
            .iter()
            .any(|p| cand == *p || cand.starts_with(&format!("{p}/")))
    }

    fn check_cancel(&mut self) {
        if self
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
        {
            self.cancelled = true;
        }
    }

    fn tick(&mut self, path: &Path) {
        self.entries += 1;
        if self.entries.is_multiple_of(CANCEL_CHECK_EVERY) {
            self.check_cancel();
        }
        if self.entries.is_multiple_of(PROGRESS_EVERY) {
            (self.progress)(Progress {
                entries: self.entries,
                bytes: self.bytes,
                current_path: path.to_path_buf(),
            });
        }
    }

    fn ext_id(&mut self, name: &str) -> u16 {
        let ext: Box<str> = Path::new(name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
            .into();
        if ext.is_empty() {
            return 0;
        }
        if let Some(&id) = self.ext_index.get(&ext) {
            return id;
        }
        let Ok(id) = u16::try_from(self.exts.len()) else {
            return 0;
        };
        self.ext_index.insert(ext.clone(), id);
        self.exts.push(ext);
        self.ext_stats.push(ExtStat::default());
        id
    }

    fn walk_dir(&mut self, dir: &Path, dir_id: NodeId, depth: u16) {
        if self.cancelled {
            return;
        }
        let rd = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => {
                // 目录本身已入树；读不了就把错误记在它身上
                self.error(dir_id);
                return;
            }
        };
        for entry in rd {
            if self.cancelled {
                break;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    self.error(dir_id);
                    continue;
                }
            };
            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => {
                    self.error(dir_id);
                    continue;
                }
            };
            let ft = meta.file_type();
            // shortcut: 名字统一存 lossy UTF-8；非 UTF-8 路径可扫可显示，但不能用于文件操作
            let name: Box<str> = entry.file_name().to_string_lossy().into();
            let child_depth = depth.saturating_add(1);
            let mtime = mtime_of(&meta);

            if ft.is_symlink() {
                self.tick(&path);
                self.push_node(Node {
                    name,
                    parent: dir_id,
                    children: Vec::new(),
                    size: 0,
                    own_size: 0,
                    file_count: 0,
                    dir_count: 0,
                    mtime,
                    kind: NodeKind::Symlink,
                    ext_id: 0,
                    depth: child_depth,
                    error_count: 0,
                });
                continue;
            }

            if ft.is_dir() {
                if !self.options.cross_filesystems {
                    if let (Some(root_dev), Some(dev)) = (self.root_dev, device_of(&meta)) {
                        if dev != root_dev {
                            continue;
                        }
                    }
                }
                if self.is_excluded(&path) {
                    continue;
                }
                self.tick(&path);
                let id = self.push_node(Node {
                    name,
                    parent: dir_id,
                    children: Vec::new(),
                    size: 0,
                    own_size: 0,
                    file_count: 0,
                    dir_count: 0,
                    mtime,
                    kind: NodeKind::Dir,
                    ext_id: 0,
                    depth: child_depth,
                    error_count: 0,
                });
                self.walk_dir(&path, id, child_depth);
                continue;
            }

            // 普通文件 / fifo / socket / 设备：按文件处理，大小取 metadata.len()
            let size = if let Some(key) = hardlink_key(&meta) {
                if self.seen_links.insert(key, ()).is_some() {
                    0
                } else {
                    meta.len()
                }
            } else {
                meta.len()
            };
            self.bytes += size;
            self.nodes[dir_id as usize].own_size += size;
            self.tick(&path);
            let ext_id = self.ext_id(&name);
            self.push_node(Node {
                name,
                parent: dir_id,
                children: Vec::new(),
                size,
                own_size: size,
                file_count: 0,
                dir_count: 0,
                mtime,
                kind: NodeKind::File,
                ext_id,
                depth: child_depth,
                error_count: 0,
            });
            let stat = &mut self.ext_stats[ext_id as usize];
            stat.bytes += size;
            stat.files += 1;
        }
    }
}

fn mtime_of(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

#[cfg(unix)]
fn device_of(meta: &fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.dev())
}

#[cfg(not(unix))]
fn device_of(_meta: &fs::Metadata) -> Option<u64> {
    // shortcut: Windows 未检测跨卷，cross_filesystems 选项只在 unix 生效
    None
}

#[cfg(unix)]
fn hardlink_key(meta: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    (meta.nlink() > 1).then(|| (meta.dev(), meta.ino()))
}

/// 是否做了硬链接去重（上报给 UI，避免用户把虚高的大小当成真占用）
#[cfg(unix)]
const HARDLINK_DEDUP: bool = true;
#[cfg(not(unix))]
const HARDLINK_DEDUP: bool = false;

// shortcut: Windows 稳定版 std 无法读 nlink / file_index（MetadataExt::file_index 仍是
// unstable 的 windows_by_handle），这里不做去重 —— 硬链接会重复计数（pnpm store、WinSxS
// 场景会虚高）。将来换 windows-sys GetFileInformationByHandle 或 std 稳定后再升级。
#[cfg(not(unix))]
fn hardlink_key(_meta: &fs::Metadata) -> Option<(u64, u64)> {
    None
}
