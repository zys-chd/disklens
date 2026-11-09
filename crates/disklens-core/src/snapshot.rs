use std::collections::HashMap;
use std::fs;
use std::io::{self, BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};

use crate::model::{NodeKind, Tree};
use crate::query::{top_files, TopFile};

const MIB: i64 = 1024 * 1024;
const SNAPSHOT_FILES: usize = 20_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub v: u32,
    pub root_path: String,
    pub created_at: i64,
    pub total_bytes: u64,
    /// 只含 size >= 1MiB 的目录
    pub dirs: Vec<(String, u64)>,
    /// Top 20000 大文件
    pub files: Vec<(String, u64)>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMeta {
    pub path: PathBuf,
    pub root_path: String,
    pub created_at: i64,
    pub total_bytes: u64,
    pub entries: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum SnapError {
    #[error("IO 错误 {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("快照格式错误 {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

fn io_err(path: &Path, source: io::Error) -> SnapError {
    SnapError::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub fn default_snapshot_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("disklens")
        .join("snapshots")
}

pub fn save_snapshot(tree: &Tree) -> Result<PathBuf, SnapError> {
    save_snapshot_to(tree, &default_snapshot_dir())
}

pub fn save_snapshot_to(tree: &Tree, dir: &Path) -> Result<PathBuf, SnapError> {
    fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    let snap = snapshot_from_tree(tree);

    // DFS 一次拿全路径，避免每个目录重走祖先链
    let mut dirs_out = Vec::new();
    let mut stack = vec![(tree.root, tree.root_path.clone())];
    while let Some((id, path)) = stack.pop() {
        let n = tree.node(id);
        if n.kind == NodeKind::Dir && n.size >= MIB as u64 {
            dirs_out.push((path.to_string_lossy().into_owned(), n.size));
        }
        for &c in &n.children {
            stack.push((c, path.join(&*tree.node(c).name)));
        }
    }
    let files_out: Vec<(String, u64)> = top_files(tree, SNAPSHOT_FILES, None)
        .into_iter()
        .map(|f: TopFile| (f.path.to_string_lossy().into_owned(), f.size))
        .collect();

    let snap = Snapshot {
        dirs: dirs_out,
        files: files_out,
        ..snap
    };
    let slug = slug_of(&tree.root_path);
    let path = dir.join(format!("{slug}-{}.json.gz", snap.created_at));
    let file = fs::File::create(&path).map_err(|e| io_err(&path, e))?;
    let mut enc = GzEncoder::new(BufWriter::new(file), Compression::default());
    // shortcut: 同一根目录同一秒重复保存会覆盖同名文件；清理旧快照的界面尚未做
    serde_json::to_writer(&mut enc, &snap).map_err(|e| SnapError::Json {
        path: path.clone(),
        source: e,
    })?;
    enc.finish().map_err(|e| io_err(&path, e))?;
    Ok(path)
}

fn snapshot_from_tree(tree: &Tree) -> Snapshot {
    Snapshot {
        v: 1,
        root_path: tree.root_path.to_string_lossy().into_owned(),
        created_at: now_secs(),
        total_bytes: tree.root_node().size,
        dirs: Vec::new(),
        files: Vec::new(),
    }
}

fn slug_of(root: &Path) -> String {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "root".into());
    let slug: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "root".into()
    } else {
        slug.to_string()
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

pub fn load_snapshot(path: &Path) -> Result<Snapshot, SnapError> {
    let file = fs::File::open(path).map_err(|e| io_err(path, e))?;
    let mut dec = GzDecoder::new(BufReader::new(file));
    let mut json = String::new();
    dec.read_to_string(&mut json).map_err(|e| io_err(path, e))?;
    serde_json::from_str(&json).map_err(|e| SnapError::Json {
        path: path.to_path_buf(),
        source: e,
    })
}

pub fn list_snapshots(dir: &Path) -> Result<Vec<SnapshotMeta>, SnapError> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(dir, e)),
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "gz") {
            continue;
        }
        // shortcut: 损坏的快照文件被跳过而不是报错，避免单个坏文件打不开列表
        let Ok(snap) = load_snapshot(&path) else {
            continue;
        };
        out.push(SnapshotMeta {
            path,
            root_path: snap.root_path,
            created_at: snap.created_at,
            total_bytes: snap.total_bytes,
            entries: snap.dirs.len() + snap.files.len(),
        });
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.created_at));
    Ok(out)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRow {
    pub path: String,
    pub size_a: u64,
    pub size_b: u64,
    pub delta: i64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    pub grown: Vec<DiffRow>,
    pub shrunk: Vec<DiffRow>,
    pub grown_bytes: u64,
    pub shrunk_bytes: u64,
}

fn delta_i64(a: u64, b: u64) -> i64 {
    let a = i64::try_from(a).unwrap_or(i64::MAX);
    let b = i64::try_from(b).unwrap_or(i64::MAX);
    b.saturating_sub(a)
}

/// 按路径 join；只报 |delta| >= 1MiB。A 有 B 无为删除（负数），反之为新增。
pub fn diff_snapshots(a: &Snapshot, b: &Snapshot, limit: usize) -> Diff {
    let mut map: HashMap<&str, (Option<u64>, Option<u64>)> =
        HashMap::with_capacity(a.dirs.len() + a.files.len() + b.dirs.len() + b.files.len());
    for (path, size) in &a.dirs {
        map.entry(path).or_default().0 = Some(*size);
    }
    for (path, size) in &a.files {
        map.entry(path).or_default().0 = Some(*size);
    }
    for (path, size) in &b.dirs {
        map.entry(path).or_default().1 = Some(*size);
    }
    for (path, size) in &b.files {
        map.entry(path).or_default().1 = Some(*size);
    }

    let mut out = Diff::default();
    for (path, (sa, sb)) in map {
        let (sa, sb) = (sa.unwrap_or(0), sb.unwrap_or(0));
        let delta = delta_i64(sa, sb);
        if delta.unsigned_abs() < MIB as u64 {
            continue;
        }
        let row = DiffRow {
            path: path.to_string(),
            size_a: sa,
            size_b: sb,
            delta,
        };
        if delta > 0 {
            out.grown_bytes += delta as u64;
            out.grown.push(row);
        } else {
            out.shrunk_bytes += delta.unsigned_abs();
            out.shrunk.push(row);
        }
    }
    out.grown
        .sort_by(|x, y| y.delta.cmp(&x.delta).then_with(|| x.path.cmp(&y.path)));
    out.shrunk
        .sort_by(|x, y| x.delta.cmp(&y.delta).then_with(|| x.path.cmp(&y.path)));
    out.grown.truncate(limit);
    out.shrunk.truncate(limit);
    out
}
