# T1 — Rust 核心引擎交付报告

日期：2026-11-09/10 ｜ crate：`crates/disklens-core` ｜ 验证机：Arch Linux，rustc/cargo 1.99.1

## ① 做了什么

**工作区与模块**
- 根 `Cargo.toml`：workspace，`members = ["crates/disklens-core"]`（T2 再自行加 `src-tauri`）。
- `src/lib.rs` 统一导出；`model.rs`（Node/Tree/NodeKind/ExtStat/ScanStats + `Tree::validate`）、`scan.rs`、`aggregate.rs`、`query.rs`、`snapshot.rs`、`guard.rs`、`ops.rs`。
- `examples/scan.rs`：`cargo run --release --example scan -- <path>`，打印总大小/条目数/错误数/耗时/峰值 RSS（VmHWM），并与 `du -sB1`、`du --apparent-size` 对照。
- `scripts/bench.sh [path] [runs]`：3 次取数，输出耗时/峰值 RSS/du 误差；不依赖 GNU `time`，RSS 由 example 读 `/proc/self/status`。

**扫描（scan.rs）**
- 自实现 `std::fs::read_dir` DFS 前序遍历建 arena（未用 jwalk/rayon，见缺口）：先序保证 `child id > parent id`，供逆序聚合。
- 符号链接：`DirEntry::metadata()` 不跟随，`NodeKind::Symlink`、`size=0`、不进入；环不会 hang。
- 硬链接：unix `nlink>1` 时按 `(st_dev, st_ino)` 去重，第二个及以后 `size=0`；Windows 分支用 `file_index()`/`volume_serial_number()`（cfg，未在 Windows 实机验证）。目录不参与去重。
- 跨文件系统：unix 比较子目录 `st_dev` 与根设备，`cross_filesystems=false`（默认）整个挂载点跳过；`/proc` `/sys` `/dev` 内置排除，但直接扫这些根时不会把根自己排除。
- 错误：`read_dir`/`entry`/`metadata` 失败不 panic，记到最近可读目录的 `error_count`，扫描继续；`stats.errors` 为全树合计。根不存在返回 `NotFound`，根是文件返回 `NotADirectory`。
- 取消：`Arc<AtomicBool>`，每 4096 条检查；取消后返回已扫部分，`stats.cancelled=true`，部分树仍过不变量。进度回调每 1024 条一次，core 自身不节流、不 sleep。
- 非 UTF-8 文件名 `to_string_lossy` 存 `Box<str>`，不 panic；扩展名小写 intern，无扩展名固定 id=0。

**聚合（aggregate.rs / model.rs）**
- 逆序单趟；`own_size` = 直属文件字节，目录 `size = own_size + Σ 子目录.size`，`file_count`/`dir_count` 同规则（文件节点 `file_count` 按 §3 公式存 0）。
- `Tree::validate()` 全量校验 I1/I2/I4（parent 链、parent<id、子节点回指、计数），有测试故意破坏 size/parent 证明断言非空转。**I1 的解释**：直属文件同时是 `children` 成员，若按字面 `own_size + Σ 全部 children` 会把直属文件算两次；实现取“own_size + Σ 子目录”，既保留 ownSize 语义又满足可验证的不变量（见缺口）。

**查询（query.rs）**
- `children(tree, node, &ChildQuery{sort_by,desc,offset,limit,min_size,filter}) -> ChildPage{total,items}`。
- `sortBy` = Size/Name/Files/Mtime；`desc` 反转；同值以 name/id 兜底保证确定性；`filter` 文件名子串、`to_lowercase` 不区分大小写；`min_size` 作用于 `size`；`offset` 越界返回空；`limit` 上限 5000；`total` 为过滤后分页前总数。
- `top_files(limit,min_size)`：单趟线性 + `BinaryHeap<Reverse<(u64,NodeId)>>` 小顶堆，结果按大小降序，路径由祖先链拼出。
- `type_stats(topN)`：按 intern 的 ext_id 聚合 bytes/files，按 bytes 降序、同值 ext 升序，`""` 表示无扩展名。

**快照（snapshot.rs）**
- `save_snapshot`（默认 `~/.local/share/disklens/snapshots`）/`save_snapshot_to`：gzip JSON，字段 `v/rootPath/createdAt/totalBytes/dirs/files`（camelCase），`dirs` 只收 ≥1MiB，`files` 为 Top 20000，文件名 `<root-slug>-<unix秒>.json.gz`。
- `load_snapshot`、`list_snapshots`（按 createdAt 降序，缺目录返回空）、`diff_snapshots`：按路径 join，A 有 B 无=负数、反之正数，`i64` saturating 计算，只报 `|delta| >= 1MiB`，`grownBytes/shrunkBytes` 为截断前全量。

**护栏（guard.rs）**
- `is_protected(path, home) -> Option<&'static str>`，返回中文拒绝原因。
- 覆盖：`/` 与任意盘符根、`/System /usr /etc /bin /lib /var/lib /boot`、Windows `C:\Windows`、`Program Files`、`%SystemRoot%`、home 本身、`Projects`、`Documents`、`.ssh`、`.hermes`、Obsidian（组件名不区分大小写）。
- `..` 组件、空路径直接拒；目标与路径均 canonicalize（不存在的目标规范化父目录再拼名字），同时做“目标在受保护路径内”和“目标是受保护路径的上级”双向保护。home 本身只保护自身/上级，home 下普通文件（Downloads/Music/…）可删；敏感子目录连子项一起保护。

**文件操作（ops.rs）**
- `move_to_trash`：逐项先 guard，再 `trash::delete`；失败只记 failed，**没有任何永久删除回退**。
- `delete_permanent(paths, confirm_text, home)`：逐项 guard + `confirm_text == basename`；不匹配项失败且不动；目录 `remove_dir_all`、文件/符号链接 `remove_file`（链接只删自身）；返回 `ok: Vec<PathBuf>` 与 `failed: [{path,error}]`。

**测试**：6 个集成测试文件 + 共享夹具，62 个用例，覆盖 04 §2 全部夹具场景与 §3 全部边界（见下）。

## ② 验证证据（均为本机真实输出）

### 测试与静态检查

```
$ cargo test -p disklens-core
     Running tests/guard_tests.rs      test result: ok. 10 passed; 0 failed
     Running tests/invariants.rs       test result: ok.  3 passed; 0 failed
     Running tests/ops_tests.rs        test result: ok.  9 passed; 0 failed
     Running tests/query_tests.rs      test result: ok. 11 passed; 0 failed; finished in 0.38s
     Running tests/scan_fixture.rs     test result: ok. 20 passed; 0 failed; finished in 0.11s
     Running tests/snapshot_tests.rs   test result: ok.  9 passed; 0 failed
   Doc-tests                             0 passed
# 合计 62 passed / 0 failed

$ cargo clippy -p disklens-core --all-targets
    Finished `dev` profile ...        # 0 warning

$ cargo fmt -p disklens-core --check
FMT_OK
```

覆盖点：3 层精确 size、I1/I2、I4 parent 链、硬链接只计一次、符号链接/环、000 权限目录、非 UTF-8、中文/emoji/255 字节名、0B/1GiB 稀疏、40 层深路径、跨设备（/dev→/dev/shm，不存在则 skip）、5000 文件分页/offset 越界/limit 5000、取消后部分树不变量、根不存在/根是文件、空目录/纯目录、exclude、空格#%中文路径、mtime、`du -sB1` 夹具误差 <0.5%；查询四种排序双向、mtime 同值确定性、filter 大小写、minSize、top_files、type_stats intern；快照 roundtrip/gzip/目录阈值/diff 增删改/阈值/limit/`u64::MAX` 溢出；guard 全清单+父目录+symlink canonicalize+Obsidian；ops confirm 不匹配、部分成功、guard 拒绝、回收站失败不回退、符号链接不伤目标。

### 性能与 du 对照

`scripts/bench.sh /home/zys/Projects 3`（含 cargo 构建后的 release example）：

```
path      /home/zys/Projects
du -sB1   4158709760
runs      3
run 1: wall=271ms scan=92ms  peak_rss=9092KB entries=47450 bytes=4032208162 vs_du=-3.042% vs_du_apparent=-0.000%
run 2: wall=281ms scan=103ms peak_rss=9100KB entries=47450 bytes=4032208162 vs_du=-3.042% vs_du_apparent=-0.000%
run 3: wall=268ms scan=93ms  peak_rss=9092KB entries=47450 bytes=4032208162 vs_du=-3.042% vs_du_apparent=-0.000%
```

`./target/release/examples/scan /home/zys`（716,475 条目，含 1 个不可读目录）：

```
total bytes 48671337677
entries     716475
errors      1
elapsed ms  1447
peak rss kb 97572
du -sB1     50437701632
vs du       -3.502%
du apparent 48672177114
vs apparent -0.002%
```

- 扫 `~/Projects`（4.03GB / 47,450 条）：**92–103ms**，峰值 RSS **9.1MB**；04 §4 阈值 ≤2s 达标。
- 扫 `~`（48.67GB / 716,475 条）：**1.45s**，峰值 RSS **97.6MB**；04 §4 阈值 ≤8s、≤350MB 均达标。按 716k 节点 97.6MB 外推 100 万节点约 136MB。
- `wall` 比 `scan` 大是因为 example 还会起两个 `du` 子进程。
- `du -sB1` 差 -3.0%/-3.5% 是**分配块 vs 逻辑大小**的预期差异（04 §7 已声明按 `st_size`）；对 `du --apparent-size` 的误差是 **0.000%/-0.002%**（数 GB 上差几百字节），证明扫描本身没有漏算/重复。
- 夹具测试 `tree_size_matches_du_within_half_percent_on_fixture` 用 9×1MiB 文件断言 `du -sB1` 误差 ≤0.5%，通过。

## ③ 已知缺口与 shortcut

1. **没有用 jwalk/rayon**：单线程 `read_dir` + 顺序 `stat`，716k 条目 1.45s，已满足 04 §4 阈值；冷缓存 HDD 或千万级目录时再换成 jwalk 并行元数据（升级点）。`Cargo.toml` 因此未引入这两个依赖。
2. **I1 语义**：按“own_size + Σ 子目录.size”实现（直属文件已是 children，按字面会把直属文件算两次）。I2 与 §3 公式一致。T2 若直接展示 `NodeView.ownSize`，目录值为直属文件字节，符合 UI 语义。
3. **文件节点 `file_count=0`**：按 §3 聚合公式存储；T2 转 `NodeView.files` 时若文件行需要显示 1，请在命令层补 1。
4. **`du -sB1` 的 0.5% 验收线达不到**：docs/04 §4 写“与 du -sB1 误差 <0.5%”，但本工具按逻辑大小（04 §7 明确），du 按分配块；47k 小文件场景差 3.0%，1M 文件场景差 3.5%。与 `du --apparent-size` 差 0.000%。属文档指标与 §7 冲突，需要产品/主控决定验收口径，不是扫描 bug。
5. **follow_symlinks**：IPC 有该选项，v1 传 `true` 直接返回 `FollowSymlinksUnsupported`，不静默忽略。
6. **Windows**：硬链接 `file_index()` cfg 代码已写但未在 Windows 编译/实机验证；跨卷检测在 Windows 未实现（`device_of` 返回 None），`cross_filesystems` 实际只在 unix 生效。
7. **非 UTF-8 文件名**：名称存 lossy UTF-8，可扫描/展示，但 `node_path` 拼出的路径不能用于删除；升级需把 `Node.name` 改为 `OsString`（会改变模型契约）。
8. **快照**：同一根目录同一秒重复保存会覆盖同名文件；损坏的快照文件在 `list_snapshots` 中被跳过而不是报错。
9. **guard 启发式**：Obsidian 按路径组件名 `Obsidian` 判断（不区分大小写），非默认安装/改名仓库会漏；目标不存在且父目录也不存在时只做词法绝对化，可能绕过符号链接规范化。
10. **delete_permanent 多选语义**：逐项校验 basename；不匹配的项失败、匹配的项照删。UI 多选不同名目标时应逐个确认，或只允许单选，避免“部分删除”。
11. **系统目录保护范围比 §6 字面更严**：`/usr`、`/etc` 等的子项也拒绝（防误删系统文件）；home 本身只保护自身和上级，home 下普通文件可删。
12. **测试环境相关**：`/dev/shm` 跨设备测试、000 权限测试在不满足条件时会打印 skip 并返回（本机均实际执行并通过）；回收站测试只断言“成功则消失 / 失败则保留”，不依赖桌面回收站可用性。
13. **平台覆盖**：只在 Linux 实跑；macOS/Windows 的 guard 盘符、`trash`、硬链接分支未实机验证。

## 文件清单

- `Cargo.toml`（workspace）、`Cargo.lock`（cargo 生成）
- `crates/disklens-core/{Cargo.toml,src/{lib,model,scan,aggregate,query,snapshot,guard,ops}.rs,examples/scan.rs}`
- `crates/disklens-core/tests/{support/mod.rs,scan_fixture.rs,query_tests.rs,guard_tests.rs,ops_tests.rs,snapshot_tests.rs,invariants.rs}`
- `scripts/bench.sh`、`.gitignore`
- `docs/reports/T1-core.md`（本文件）

未触碰 `app/`、`src-tauri/`。
