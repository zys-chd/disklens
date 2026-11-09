//! 真 IPC 往返：用 Tauri 的 MockRuntime 把 InvokeRequest 打进真实命令层，
//! 校验 camelCase 参数/字段名、扫描线程、树查询、快照、护栏。

use std::time::Duration;

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::webview::InvokeRequest;
use tauri::{WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use disklens_lib::state::AppState;

fn invoke(w: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, String> {
    let res = tauri::test::get_ipc_response(
        w,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    );
    match res {
        Ok(b) => Ok(b.deserialize().expect("响应不是合法 JSON")),
        Err(e) => Err(e.to_string()),
    }
}

fn wait_tree(w: &WebviewWindow<MockRuntime>, scan_id: &str) -> Value {
    for _ in 0..100 {
        let q = json!({
            "scanId": scan_id, "nodeId": 0,
            "sortBy": "size", "desc": true, "offset": 0, "limit": 50
        });
        if let Ok(v) = invoke(w, "list_children", q) {
            return v;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("扫描 5s 内未完成");
}

#[test]
fn ipc_contract_roundtrip() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("XDG_DATA_HOME", data.path());
    // 扫描会写 rules.json 到 config_dir，别污染真实用户配置
    std::env::set_var("XDG_CONFIG_HOME", data.path());

    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("big.bin"), vec![0u8; 1_500_000]).unwrap();
    std::fs::create_dir(root.path().join("sub")).unwrap();
    std::fs::write(root.path().join("sub/small.txt"), b"hello").unwrap();

    let app = mock_builder()
        .manage(AppState::default())
        .invoke_handler(disklens_lib::ipc_handler())
        .build(mock_context(noop_assets()))
        .unwrap();
    let w = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .unwrap();

    // start_scan：camelCase 的 options，立刻返回 scanId
    let start = invoke(
        &w,
        "start_scan",
        json!({ "path": root.path(), "options": { "crossFilesystems": false } }),
    )
    .unwrap();
    let scan_id = start["scanId"].as_str().expect("缺 scanId").to_string();

    // 树就绪后：分页 + NodeView 字段
    let page = wait_tree(&w, &scan_id);
    assert_eq!(page["total"], 2, "根目录应有 2 个子项");
    let items = page["items"].as_array().unwrap();
    assert_eq!(items[0]["name"], "big.bin", "默认按 size 降序");
    assert_eq!(items[0]["size"], 1_500_000u64);
    assert_eq!(items[0]["ownSize"], 1_500_000u64);
    assert_eq!(items[0]["kind"], "file");
    assert_eq!(items[0]["hasChildren"], false);
    assert!(items[0].get("errorCount").is_some());
    // 分级：扫描完成后节点自带 safety，汇总接口可用
    let level = items[0]["safety"].as_str().expect("扫描后节点应带 safety");
    assert!(
        matches!(level, "safe" | "careful" | "keep" | "unknown"),
        "{level}"
    );
    let stats = invoke(&w, "safety_stats", json!({ "scanId": scan_id })).unwrap();
    assert!(stats["safeBytes"].is_u64() && stats["carefulBytes"].is_u64());
    assert!(stats["stats"].is_array());
    let safe_only = invoke(
        &w,
        "list_children",
        json!({
            "scanId": scan_id, "nodeId": 0,
            "sortBy": "size", "desc": true, "offset": 0, "limit": 50,
            "safety": ["safe"]
        }),
    )
    .unwrap();
    for it in safe_only["items"].as_array().unwrap() {
        assert_eq!(it["safety"], "safe");
    }

    // get_node / node_path / top_files / type_stats
    let node = invoke(&w, "get_node", json!({ "scanId": scan_id, "nodeId": 0 })).unwrap();
    assert_eq!(node["id"], 0);
    assert_eq!(node["dirs"], 1);
    assert_eq!(node["files"], 2);

    let big_id = items[0]["id"].as_u64().unwrap();
    let path = invoke(&w, "node_path", json!({ "scanId": scan_id, "nodeId": big_id })).unwrap();
    assert!(path.as_str().unwrap().ends_with("big.bin"), "{path}");

    let top = invoke(
        &w,
        "top_files",
        json!({ "scanId": scan_id, "limit": 5, "minSize": 1 }),
    )
    .unwrap();
    let top = top.as_array().unwrap();
    assert_eq!(top[0]["name"], "big.bin");
    assert!(top[0]["path"].as_str().unwrap().ends_with("big.bin"));

    let types = invoke(&w, "type_stats", json!({ "scanId": scan_id, "topN": 5 })).unwrap();
    assert!(types.as_array().unwrap().iter().any(|t| t["ext"] == "bin"));

    // 快照：保存 → 列表 → 自比对
    let saved = invoke(&w, "save_snapshot", json!({ "scanId": scan_id })).unwrap();
    let snap_path = saved["path"].as_str().unwrap().to_string();
    assert!(std::path::Path::new(&snap_path).exists());

    let snaps = invoke(&w, "list_snapshots", json!({})).unwrap();
    assert_eq!(snaps.as_array().unwrap().len(), 1);
    assert_eq!(snaps[0]["rootPath"], root.path().to_string_lossy().as_ref());
    assert!(snaps[0].get("createdAt").is_some());

    let diff = invoke(
        &w,
        "diff_snapshots",
        json!({ "aPath": snap_path, "bPath": snap_path, "limit": 10 }),
    )
    .unwrap();
    assert_eq!(diff["grown"].as_array().unwrap().len(), 0);
    assert_eq!(diff["shrunkBytes"], 0);

    // 平台信息
    let info = invoke(&w, "platform_info", json!({})).unwrap();
    assert_eq!(info["os"], "linux");
    assert!(info["homeDir"].as_str().unwrap().starts_with('/'));

    // 失败路径：无效 scanId、护栏拒绝
    let err = invoke(&w, "get_node", json!({ "scanId": "scan-404", "nodeId": 0 })).unwrap_err();
    assert!(err.contains("扫描不存在"), "{err}");

    let denied = invoke(
        &w,
        "delete_permanent",
        json!({ "paths": ["/usr/lib"], "confirmText": "lib" }),
    )
    .unwrap();
    assert_eq!(denied["ok"].as_array().unwrap().len(), 0);
    assert!(!denied["failed"].as_array().unwrap().is_empty());
    assert!(std::path::Path::new("/usr/lib").exists(), "护栏必须拦住");

    let denied = invoke(&w, "move_to_trash", json!({ "paths": ["/usr"] })).unwrap();
    assert_eq!(denied["ok"].as_array().unwrap().len(), 0);
    assert!(!denied["failed"].as_array().unwrap().is_empty());

    let err = invoke(&w, "cancel_scan", json!({ "scanId": "scan-404" })).unwrap_err();
    assert!(err.contains("扫描不存在"), "{err}");
}
