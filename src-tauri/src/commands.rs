//! IPC 命令层：只做参数校验 → 调 core → 转 IPC 视图 / 发事件。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use disklens_core::{
    children, diff_snapshots as diff, list_snapshots as list_snaps, load_snapshot, move_to_trash as trash,
    node_path as core_node_path, save_snapshot as save_snap, scan_with, top_files as core_top_files,
    type_stats as core_type_stats, ChildQuery, NodeId, NodeKind, OpResult, ScanOptions, SortBy, Tree,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use crate::state::AppState;

const PROGRESS_EVERY: Duration = Duration::from_millis(200);

// ---------- IPC 视图 ----------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeView {
    id: NodeId,
    name: String,
    size: u64,
    own_size: u64,
    files: u32,
    dirs: u32,
    mtime: i64,
    kind: NodeKind,
    has_children: bool,
    error_count: u32,
}

fn view(tree: &Tree, id: NodeId) -> NodeView {
    let n = tree.node(id);
    NodeView {
        id,
        name: n.name.to_string(),
        size: n.size,
        own_size: n.own_size,
        files: n.file_count,
        dirs: n.dir_count,
        mtime: n.mtime,
        kind: n.kind,
        has_children: !n.children.is_empty(),
        error_count: n.error_count,
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    name: String,
    mount_point: String,
    total_bytes: u64,
    available_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    scan_id: String,
    entries: u64,
    bytes: u64,
    current_path: String,
    elapsed_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    scan_id: String,
    root_path: String,
    root: NodeView,
    elapsed_ms: u64,
    entries: u64,
    errors: u64,
    cancelled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanErrorPayload {
    scan_id: String,
    message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    os: &'static str,
    home_dir: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanOptionsIn {
    cross_filesystems: Option<bool>,
    follow_symlinks: Option<bool>,
    exclude: Option<Vec<String>>,
}

fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "无法确定主目录".to_string())
}

// ---------- 扫描 ----------

#[tauri::command]
pub fn list_drives() -> Vec<Drive> {
    let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else {
        return Vec::new();
    };
    disks
        .iter()
        .map(|d| {
            let name = d.name().to_string_lossy();
            Drive {
                name: if name.is_empty() {
                    d.mount_point().to_string_lossy().into_owned()
                } else {
                    name.into_owned()
                },
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
            }
        })
        .collect()
}

#[tauri::command(rename_all = "camelCase")]
pub fn start_scan<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
    options: Option<ScanOptionsIn>,
) -> Result<serde_json::Value, String> {
    // 同步校验根目录：无效路径直接让 invoke 失败，不依赖事件（事件可能早于前端拿到 scanId）
    let root = std::fs::canonicalize(&path).map_err(|e| format!("无法解析路径 {path}: {e}"))?;
    if !root.is_dir() {
        return Err(format!("不是目录: {}", root.display()));
    }
    let o = options.unwrap_or_default();
    let opts = ScanOptions {
        cross_filesystems: o.cross_filesystems.unwrap_or(false),
        follow_symlinks: o.follow_symlinks.unwrap_or(false),
        exclude: o
            .exclude
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect(),
    };

    let cancel = Arc::new(AtomicBool::new(false));
    let scan_id = state.begin(cancel.clone());
    let rec = state.get(&scan_id)?;
    let started = Instant::now();
    let thread_id = scan_id.clone();
    let app_thread = app.clone();

    std::thread::spawn(move || {
        let mut last_emit = Instant::now()
            .checked_sub(PROGRESS_EVERY)
            .unwrap_or_else(Instant::now);
        let result = scan_with(&root, &opts, Some(cancel), |p| {
            if last_emit.elapsed() >= PROGRESS_EVERY {
                last_emit = Instant::now();
                let _ = app_thread.emit(
                    "scan://progress",
                    ProgressPayload {
                        scan_id: thread_id.clone(),
                        entries: p.entries,
                        bytes: p.bytes,
                        current_path: p.current_path.to_string_lossy().into_owned(),
                        elapsed_ms: started.elapsed().as_millis() as u64,
                    },
                );
            }
        });
        match result {
            Ok(tree) => {
                let tree = Arc::new(tree);
                // 先落树再发 done：前端收到 done 会立刻 list_children
                *rec.tree.write().expect("tree poisoned") = Some(tree.clone());
                let _ = app_thread.emit(
                    "scan://done",
                    ScanSummary {
                        scan_id: thread_id.clone(),
                        root_path: tree.root_path.to_string_lossy().into_owned(),
                        root: view(&tree, tree.root),
                        elapsed_ms: tree.stats.elapsed_ms,
                        entries: tree.stats.entries,
                        errors: tree.stats.errors,
                        cancelled: tree.stats.cancelled,
                    },
                );
            }
            Err(e) => {
                rec.alive.store(false, Ordering::Release);
                let _ = app_thread.emit(
                    "scan://error",
                    ScanErrorPayload {
                        scan_id: thread_id,
                        message: e.to_string(),
                    },
                );
            }
        }
    });

    Ok(json!({ "scanId": scan_id }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn cancel_scan(state: State<'_, AppState>, scan_id: String) -> Result<(), String> {
    state.get(&scan_id)?.cancel.store(true, Ordering::Release);
    Ok(())
}

// ---------- 查询 ----------

#[tauri::command(async, rename_all = "camelCase")]
pub fn list_children(
    state: State<'_, AppState>,
    scan_id: String,
    node_id: NodeId,
    sort_by: SortBy,
    desc: bool,
    offset: usize,
    limit: usize,
    min_size: Option<u64>,
    filter: Option<String>,
) -> Result<serde_json::Value, String> {
    let tree = state.tree(&scan_id)?;
    if tree.nodes.get(node_id as usize).is_none() {
        return Err(format!("节点不存在: {node_id}"));
    }
    let q = ChildQuery {
        sort_by,
        desc,
        offset,
        limit,
        min_size,
        filter,
    };
    let page = children(&tree, node_id, &q);
    let items: Vec<NodeView> = page.items.iter().map(|&id| view(&tree, id)).collect();
    Ok(json!({ "total": page.total, "items": items }))
}

#[tauri::command(async, rename_all = "camelCase")]
pub fn get_node(
    state: State<'_, AppState>,
    scan_id: String,
    node_id: NodeId,
) -> Result<NodeView, String> {
    let tree = state.tree(&scan_id)?;
    if tree.nodes.get(node_id as usize).is_none() {
        return Err(format!("节点不存在: {node_id}"));
    }
    Ok(view(&tree, node_id))
}

#[tauri::command(async, rename_all = "camelCase")]
pub fn node_path(state: State<'_, AppState>, scan_id: String, node_id: NodeId) -> Result<String, String> {
    let tree = state.tree(&scan_id)?;
    if tree.nodes.get(node_id as usize).is_none() {
        return Err(format!("节点不存在: {node_id}"));
    }
    Ok(core_node_path(&tree, node_id).to_string_lossy().into_owned())
}

#[tauri::command(async, rename_all = "camelCase")]
pub fn top_files(
    state: State<'_, AppState>,
    scan_id: String,
    limit: usize,
    min_size: Option<u64>,
) -> Result<serde_json::Value, String> {
    let tree = state.tree(&scan_id)?;
    Ok(json!(core_top_files(&tree, limit, min_size)))
}

#[tauri::command(async, rename_all = "camelCase")]
pub fn type_stats(
    state: State<'_, AppState>,
    scan_id: String,
    top_n: usize,
) -> Result<serde_json::Value, String> {
    let tree = state.tree(&scan_id)?;
    Ok(json!(core_type_stats(&tree, top_n)))
}

// ---------- 快照 ----------

#[tauri::command(async, rename_all = "camelCase")]
pub fn save_snapshot(state: State<'_, AppState>, scan_id: String) -> Result<serde_json::Value, String> {
    let tree = state.tree(&scan_id)?;
    let path = save_snap(&tree).map_err(|e| e.to_string())?;
    Ok(json!({ "path": path.to_string_lossy() }))
}

#[tauri::command(async)]
pub fn list_snapshots() -> Result<serde_json::Value, String> {
    let metas = list_snaps(&disklens_core::default_snapshot_dir()).map_err(|e| e.to_string())?;
    Ok(json!(metas))
}

#[tauri::command(async, rename_all = "camelCase")]
pub fn diff_snapshots(
    a_path: String,
    b_path: String,
    limit: usize,
) -> Result<serde_json::Value, String> {
    let a = load_snapshot(Path::new(&a_path)).map_err(|e| e.to_string())?;
    let b = load_snapshot(Path::new(&b_path)).map_err(|e| e.to_string())?;
    Ok(json!(diff(&a, &b, limit)))
}

// ---------- 系统 ----------

#[tauri::command(async)]
pub fn open_path(path: String, reveal: Option<bool>) -> Result<(), String> {
    crate::open::open_path(Path::new(&path), reveal.unwrap_or(false))
}

/// 护栏在 disklens_core::ops 内部（is_protected），前端传什么都绕不过去。
#[tauri::command(async)]
pub fn move_to_trash(paths: Vec<String>) -> Result<OpResult, String> {
    let home = home()?;
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    Ok(trash(&paths, &home))
}

#[tauri::command(async, rename_all = "camelCase")]
pub fn delete_permanent(paths: Vec<String>, confirm_text: String) -> Result<OpResult, String> {
    let home = home()?;
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    Ok(disklens_core::delete_permanent(&paths, &confirm_text, &home))
}

#[tauri::command]
pub fn platform_info() -> Result<PlatformInfo, String> {
    Ok(PlatformInfo {
        os: if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        },
        home_dir: home()?.to_string_lossy().into_owned(),
    })
}

/// 前端只拿到路径字符串，不引入 @tauri-apps/plugin-dialog 依赖。
#[tauri::command(async)]
pub fn pick_folder<R: tauri::Runtime>(app: AppHandle<R>) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 契约镜像：字段名/类型必须与 app/src/lib/types.ts 的 NodeView 逐字一致。
    #[test]
    fn node_view_matches_ipc_contract() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), vec![7u8; 1024]).unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let tree = disklens_core::scan(dir.path(), &ScanOptions::default()).unwrap();
        let json = serde_json::to_value(view(&tree, tree.root)).unwrap();

        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "dirs",
                "errorCount",
                "files",
                "hasChildren",
                "id",
                "kind",
                "mtime",
                "name",
                "ownSize",
                "size"
            ]
        );
        assert_eq!(json["kind"], "dir");
        assert_eq!(json["id"], 0);
        assert_eq!(json["size"], 1024);
        assert_eq!(json["files"], 1);
        assert_eq!(json["dirs"], 1);
        assert_eq!(json["hasChildren"], true);
    }
}
