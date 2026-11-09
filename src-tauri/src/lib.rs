//! DiskLens Tauri 外壳：注册状态、插件与全部 IPC 命令。

pub mod commands;
mod open;
pub mod state;

use state::AppState;

/// 单一命令表：run() 与 tests/ipc.rs 共用，避免两处漂移。
#[doc(hidden)]
pub fn ipc_handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::list_drives,
        commands::start_scan,
        commands::cancel_scan,
        commands::list_children,
        commands::get_node,
        commands::safety_stats,
        commands::node_path,
        commands::top_files,
        commands::type_stats,
        commands::save_snapshot,
        commands::list_snapshots,
        commands::diff_snapshots,
        commands::open_path,
        commands::open_rules_file,
        commands::move_to_trash,
        commands::delete_permanent,
        commands::platform_info,
        commands::pick_folder,
    ]
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(ipc_handler())
        .run(tauri::generate_context!())
        .expect("启动 DiskLens 失败");
}
