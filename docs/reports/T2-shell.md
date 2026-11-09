# T2 — Tauri 2 外壳与命令层

> 2026-11-10 ｜ 状态：完成（GUI 交互项未能自动化验证，见 §3）

## ① 做了什么

### 新增 `src-tauri/`

| 文件 | 内容 |
|---|---|
| `Cargo.toml` | `tauri = "2"`(2.12.5) / `tauri-build = "2"` / `tauri-plugin-dialog = "2"` / `sysinfo = "0.40"`，lib 名 `disklens_lib`，依赖 `disklens-core`（path） |
| `build.rs` | `tauri_build::build()` |
| `tauri.conf.json` | `frontendDist="../app/dist"`、`devUrl="http://localhost:5173"`、`beforeDevCommand="npm --prefix ../app run dev"`、`beforeBuildCommand="npm --prefix ../app run build"`、窗口 1280×820 / min 980×600 / title `DiskLens` / label `main`；`identifier=dev.disklens.desktop`（`*.app` 结尾会告警，已避开） |
| `capabilities/default.json` | `permissions: ["core:default", "dialog:default"]`，`windows: ["main"]` |
| `icons/icon.png` | 512×512 占位图标（PIL 生成） |
| `src/main.rs` | 桌面入口 → `disklens_lib::run()`，release 下隐藏控制台 |
| `src/lib.rs` | 插件/状态注册；`ipc_handler()` 为**唯一**命令表，`run()` 与集成测试共用，避免两处漂移 |
| `src/state.rs` | `AppState`：`HashMap<String, Arc<ScanRecord>>` + 插入序 `VecDeque`，**只保留最近 3 个扫描**；无效 id 返回 `扫描不存在或已释放: {id}`；树未就绪返回 `扫描尚未完成: {id}` |
| `src/commands.rs` | §4 全部 16 条命令 + `pick_folder` |
| `src/open.rs` | 三平台 `open_path` |
| `tests/ipc.rs` | 用 `tauri::test::MockRuntime` 走**真实 IPC** 的端到端往返测试 |

### 命令与契约对照

| 命令 | 说明 |
|---|---|
| `list_drives` | `sysinfo::Disks::new_with_refreshed_list()`（0.40 返回 `Result`，失败返回空表） |
| `start_scan` | 同步 canonicalize + `is_dir` 校验（坏路径直接 reject，不依赖事件，规避“事件早于 scanId 落地”竞态）→ 登记 scanId → `std::thread::spawn` 跑 `scan_with` |
| `cancel_scan` | `AtomicBool::store(true)`（core 每 4096 条轮询） |
| `list_children` | `ChildQuery`；`sortBy` 直接复用 core 的 `SortBy`（serde lowercase），`limit` 由 core 截到 `MAX_LIMIT=5000`；越界 nodeId 明确报错 |
| `get_node` / `node_path` | 越界 nodeId 报 `节点不存在` |
| `top_files` / `type_stats` | 直接转 core 的 camelCase 结构 |
| `save_snapshot` | `{path}`；`list_snapshots` 返回 core `SnapshotMeta`（`path` 为字符串）；`diff_snapshots` 载入两份快照再 diff |
| `open_path` | Linux：`gdbus` 调 `org.freedesktop.FileManager1.ShowItems`（file URI 自建百分号编码），失败回退 `xdg-open` 父目录；`reveal=false` 直接 `xdg-open`。Windows：`explorer /select,<path>`；macOS：`open -R` |
| `move_to_trash` / `delete_permanent` | 调 `core::ops`，护栏 `is_protected` 在其内部（前端无法绕过；`confirmText` 校验也在 core） |
| `platform_info` | cfg 判定 os + `dirs::home_dir()` |
| `pick_folder` | `tauri-plugin-dialog` 的 `blocking_pick_folder()`，返回路径字符串或 `null`；该 API 内部走 `run_on_main_thread`，因此命令标 `async` 放到线程池执行，不阻塞、不死锁 |

事件（payload 与 §4 逐字一致的结构体）：
- `scan://progress` `{scanId, entries, bytes, currentPath, elapsedMs}`：扫描线程内 200ms 节流（core 每 1024 条回调一次）
- `scan://done` `ScanSummary`：**先写 tree 再发事件**，前端收到即 `list_children` 不会撞空
- `scan://error` `{scanId, message}`：线程内失败时发，并把该 scan 标 alive=false

并发/阻塞：所有会做 IO 或长 CPU 的命令（查询、快照、删除、打开、选目录）都标了 `#[tauri::command(async)]`，避免占 GTK 主线程；`start_scan` / `cancel_scan` 保持同步（毫秒级）。

### 前端补 F1 缺口

- `lib/types.ts`：`Backend` 增加 `pickFolder(): Promise<string | null>`
- `lib/backend/tauri.ts`：`invoke('pick_folder')`
- `lib/backend/mock.ts`：返回固定 `/home/zys`
- `components/TopBar.tsx`：新增“选目录”按钮（选中即填路径并发起扫描）；`onDragDropEvent` 监听 `drop`，拖入路径直接扫描（`isTauri()` 守卫，jsdom 下不注册）
- `app/package.json`：`@tauri-apps/cli ^2.12.4` devDependency + `scripts.tauri`

### 其它改动

- 根 `Cargo.toml`：`members = ["crates/disklens-core", "src-tauri"]`（只此一行）
- `Cargo.lock` 随依赖更新

## ② 验证证据

依赖版本：`tauri 2.12.5`、`tauri-plugin-dialog 2.8.4`、`sysinfo 0.40.1`、`tauri-cli 2.12.4`。

### Rust 编译（无 warning）

```
$ cd /home/zys/Projects/disklens && cargo check -p disklens && cargo build -p disklens
    Checking disklens v0.1.0 (/home/zys/Projects/disklens/src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.36s
check exit=0
   Compiling disklens v0.1.0 (/home/zys/Projects/disklens/src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.73s
```

### Rust 测试（4 单测 + 1 真 IPC 集成测试）

```
$ cargo test -p disklens
running 4 tests
test state::tests::keeps_last_three_and_invalidates_old ... ok
test state::tests::unknown_id_is_an_error ... ok
test state::tests::tree_query_before_done_fails ... ok
test commands::tests::node_view_matches_ipc_contract ... ok
test result: ok. 4 passed; 0 failed

     Running tests/ipc.rs
running 1 test
test ipc_contract_roundtrip ... ok
test result: ok. 1 passed; 0 failed
```

`tests/ipc.rs` 通过 MockRuntime 把 InvokeRequest 打进真实命令层，断言的是**线上格式**而不只是函数：
`start_scan{path, options:{crossFilesystems}}` → `{scanId}`；`list_children{scanId,nodeId,sortBy,desc,offset,limit}` 返回 `total=2`、`items[0]` 含 `size/ownSize/files/dirs/mtime/kind/hasChildren/errorCount` 且按 size 降序；`get_node` 根节点 `dirs=1 files=2`；`node_path` 以 `big.bin` 结尾；`top_files{limit,minSize}` 返回 `{name,path,...}`；`type_stats{topN}` 含 `bin`；`save_snapshot` 落盘 → `list_snapshots` 有一条且 `rootPath` 正确 → `diff_snapshots` 自比对为空；`platform_info.os="linux"`；`scan-404` 报“扫描不存在”；`delete_permanent(["/usr/lib"], "lib")` 与 `move_to_trash(["/usr"])` 的 `ok` 均为空、`failed` 非空且 `/usr/lib` 仍在。

### 前端

```
$ cd app && npx tsc --noEmit && npx vitest run && npm run build
tsc: 0 errors
 Test Files  6 passed (6)
      Tests  38 passed (38)
dist/assets/index-CzrTbXh7.js   350.44 kB │ gzip: 109.43 kB
✓ built in 880ms
```

38 个既有测试全绿，未新增/修改任何测试文件。

### Tauri 构建

任务书给的 `cd app && npx tauri build --no-bundle` **失败**（原因见 §3）：

```
$ cd /home/zys/Projects/disklens/app && npx tauri build --no-bundle
thread '<unnamed>' (75521) panicked at crates/tauri-cli/src/helpers/app_paths.rs:140:5:
Couldn't recognize the current folder as a Tauri project. It must contain a `tauri.conf.json`, `tauri.conf.json5` or `Tauri.toml` file in any subfolder.
EXIT=134
```

从仓库根（`src-tauri` 就在 `./src-tauri`）调用同一个 CLI 成功：

```
$ cd /home/zys/Projects/disklens && ./app/node_modules/.bin/tauri build --no-bundle
     Running beforeBuildCommand `npm --prefix ../app run build`
> disklens-app@0.1.0 build
> tsc --noEmit && vite build
✓ 72 modules transformed.
✓ built in 863ms
   Compiling disklens v0.1.0 (/home/zys/Projects/disklens/src-tauri)
    Finished `release` profile [optimized] target(s) in 16.52s
       Built application at: /home/zys/Projects/disklens/target/release/disklens
EXIT=0
```

生成的 ACL 与配置一致：

```
$ cat src-tauri/gen/schemas/capabilities.json
{"default": {"identifier": "default", "description": "DiskLens 主窗口默认能力",
 "local": true, "windows": ["main"], "permissions": ["core:default", "dialog:default"]}}
```

### 运行时冒烟（无 xvfb，但宿主有活的 Wayland 会话）

环境里 `xvfb-run` / `Xvfb` / `weston` 都没有，`sudo` 需密码，故未装。但机器上存在 `/run/user/1000/wayland-0`，release 二进制实际把窗口开起来了：

```
$ ./target/release/disklens &   # 后台
$ ps -o pid,stat,etime,rss,comm -p <pid>
   PID STAT     ELAPSED    RSS COMMAND
 80671 Ssl        00:06 529236 disklens
$ pgrep -a -P <pid>
/usr/lib/webkit2gtk-4.1/WebKitNetworkProcess 1
/usr/lib/webkit2gtk-4.1/WebKitWebProcess 4
```

即 GTK 窗口 + WebKit 渲染进程都起来了，stderr 无输出（配置/ACL/资源加载没有报错）。约 6s 后手动结束进程。

## ③ 已知缺口 / shortcut

1. **`cd app && npx tauri build --no-bundle` 不可用**：tauri-cli 只在**当前目录及其子目录**找 `tauri.conf.json`，而本布局把 `src-tauri/` 放在仓库根、`package.json` 在 `app/`。要改只能给 `app/src-tauri` 做软链或加根 `package.json`，都超出本任务的文件所有权。请统一用仓库根入口：
   `cd /home/zys/Projects/disklens && ./app/node_modules/.bin/tauri {dev,build}`（`app` 里 `npm run tauri -- ...` 同样受此限制）。
2. **GUI 交互未自动化验证**：`pick_folder` 的 GTK 对话框、窗口拖拽（`onDragDropEvent`）、删除确认对话框都只做了代码审查 + 编译验证，没有真的点过（无 Xvfb、无法注入输入）。窗口本身确认能起（上节）。
3. **Windows/macOS 分支未运行**：`explorer /select,`、`open -R` 只有 cfg 编译期隔离，本机无交叉目标可编译验证。
4. **护栏不在 src-tauri 重复一遍**：`move_to_trash`/`delete_permanent` 直达 `core::ops`，`is_protected` 在 core 内先跑（IPC 测试已证明绕过失败）。如果再在外层加一次，会出现两条错误路径且无收益。
5. **事件是全局 emit**：`app.emit` 广播到所有 webview；当前单窗口，多窗口时要改成 `emit_to`。
6. **进度节流是叠加的**：core 每 1024 条回调 + 外层 200ms 节流，因此 <200ms 的小目录扫描只有 `done`、没有 `progress`，前端已按此实现。
7. **`pick_folder` 阻塞一个 async 线程池线程**直到用户关对话框（这是 `blocking_pick_folder` 的语义，正常）。
8. **删除类命令对超大目录仍是同步删除**（core 行为），`async` 只保证不卡 UI 主线程，没有取消能力。
9. `src-tauri/gen/`（tauri build 生成）目前是未跟踪文件；建议加进 `.gitignore`，但根 `.gitignore` 不在本任务文件所有权内，未改。
10. 快照同根同秒会覆盖同名文件（core 既有 shortcut，未改）。
11. 验证期间为跑窗口，短暂在你的 GNOME 桌面打开过一个 DiskLens 窗口，已结束进程。

## 改动清单

新增：`src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,icons/icon.png,src/{main,lib,state,commands,open}.rs,tests/ipc.rs}`

修改：`Cargo.toml`（workspace member）、`app/package.json`（cli devDep + `tauri` script）、`app/src/lib/types.ts`、`app/src/lib/backend/{tauri,mock}.ts`、`app/src/components/TopBar.tsx`、`app/package-lock.json`、`Cargo.lock`
