# T7 — 跨平台路径归一化 / 护栏 / 规则加固 交付报告

日期：2026-11-10 ｜ 范围：`crates/disklens-core/**`、`src-tauri/src/open.rs` ｜ 验证机：Arch Linux，rustc/cargo 1.99.1（另装 rustup stable 做交叉检查）

## ① 做了什么

### 1. 路径归一化（新增 `crates/disklens-core/src/path.rs`）

- `pub fn norm_path_str(p: &Path) -> String`：`\`→`/`；去 `\\?\` 前缀（`\\?\UNC\srv\share` → `//srv/share`）；盘符统一大写（`c:` → `C:`）；去掉除 `/`、`//`、`C:/` 以外的尾部 `/`。**平台无关的纯字符串变换**，所以 Windows 形状的路径在 Linux 上也能被断言。
- `case_key(s)`：macOS / Windows 折叠大小写（`CASE_INSENSITIVE_FS`），Linux 原样 —— 规则匹配侧按此策略。
- `is_root(s)` / `is_drive_abs(s)`：`/`、`//`、`C:/` 与 `C:/...` 的跨平台识别（Linux 上 `C:\a` 既不是绝对路径也不是相对当前目录）。

### 2. 规则匹配（`rules.rs`）

- `scan_one(key: &str, bound)` 接收**已归一化**的键；`full`/`base` 两个 GlobSet 都拿归一化字符串去匹配，basename 从归一化串的最后一段取（不再用 `path.file_name()`，Windows 形状在 Linux 上也能取对）。
- `classify()` 的祖先链改成按归一化字符串分段展开（`/a/b/c` → `/`、`/a`、`/a/b`、`/a/b/c`），`C:\a\b` 在 Linux 上也有正确祖先链；顺带修掉 `"/"` 被 split 成两段空串导致根节点退化为 unknown 的边角。
- `expand_pattern()`：`~` 用 home 展开、`%VAR%` 无条件展开（未设置的环境变量原样保留，规则不会误匹配），最后统一归一化 + 大小写折叠；相对模式（`*/node_modules`）仍保持相对，不拼 home。
- `expand_path()`：`C:\a` 视为「已绝对」不再拼 cwd。

### 3. 护栏（`guard.rs`）

- 比较全部改为归一化字符串键（`绝对化 → canonicalize → 归一化 → 折叠大小写`），`is_root`、同路径、祖先判定都基于字符串，去掉了 `Path::components` 在 Windows 形状上的依赖。
- `..` 检查改为对归一化分段做，`C:\a\..\Windows` 也拦得住；`~` / `~\x` 先展开成 home 再判定（`is_protected("~")` → `主目录不能删除`）。
- 盘符根 `C:\`、`C:/`、`\\?\C:\` 统一识别为 `系统根目录受保护`；`C:\Windows` / `C:\Program Files*` / `C:\Program Files (x86)` / `%SystemRoot%`（`windir` 兜底）恒在被保护列表（跨平台都生效，Linux 上不会出现这种形状的真实路径）。
- home 只保护自身、不保护子项 —— 所以 `%LOCALAPPDATA%\Temp`（在 home 之下）仍然清得掉，不会被护栏误拦。
- 大小写：护栏**在所有平台**都不区分大小写（fail-closed，宁可误拒），代码里写明是为删除安全做的取舍。

### 4. 规则表平台差异（`rules.rs::default_rules`）

- **Windows**（`cfg(windows)`）：`C:\Windows\Temp*`（careful，`needs_root=true`）、`%LOCALAPPDATA%\Temp*`（careful）、`%LOCALAPPDATA%\Microsoft\Windows\INetCache\*`（safe）放在系统段最前面；`%APPDATA%\*`（keep）插在用户数据段（`~` 之前），避免抢走 `%LOCALAPPDATA%` 的具体规则。
  - **顺序陷阱**：`C:\Windows\Temp*` 必须排在 `C:\Windows\*` 之前，否则祖先的 keep 会把 bound 压到 1，careful 永远进不来。
  - `%USERPROFILE%\Downloads\*` 未重复添加：`~/Downloads/*` 已经把 `~` 展开成 `%USERPROFILE%`，同一条规则。
- **macOS**（`cfg(target_os = "macos")`）：新增 `/Library/Caches/*`（careful，`needs_root`，排在 `/Library/*` 之前，否则被 keep 吃掉）、`~/Library/Developer/Xcode/iOS DeviceSupport/*`（careful）、`~/Library/Application Support/*`（keep）；`~/Library/Logs/*` 由 safe 改 careful、`DerivedData/*` 由 careful 改 safe（按任务口径）。
- Linux 的 77 条规则**一条没动**，条数与顺序都有测试锚定。

### 5. Windows 编译失败修复（CI 实测）

- `crates/disklens-core/src/scan.rs` 删掉 `#[cfg(windows)]` 里用 `MetadataExt::file_index()` / `volume_serial_number()` 的硬链接去重 —— 这两个 API 在稳定版 Rust 上仍是 `windows_by_handle` 不稳定特性，Windows 目标**编译直接失败**（E0658）。
- Windows 改为不去重（Unix/macOS 仍按 `(dev, ino)` 精确去重），代码内 `shortcut:` 注明 pnpm store / WinSxS 会虚高，升级路径是 `windows-sys` 的 `GetFileInformationByHandle`。
- `ScanStats` 新增 `hardlink_dedup: bool`（Unix=true / Windows=false），有测试断言该值。

### 6. `open.rs` 可测化

- 抽出 `pub enum Platform { Linux, Windows, MacOs }` 与 `pub fn open_argv(platform, path, reveal) -> Vec<String>`（纯函数，不执行）；三平台 `open_path` 只做「取 argv → 执行」，两个 cfg 版 `run` 合并成一个。
- Windows 分支把 `/` 还原成 `\` 并拼成单个参数 `"/select,C:\a\b"`（explorer 只认原生分隔符，带空格的路径也必须是同一个 argv）。
- 3 个单测在 Linux 上断言三平台 argv：`["explorer", "/select,C:\\a\\b"]`、`["open","-R","/a/b"]`、`["xdg-open","/a"]`。

### 7. 测试

新增 `crates/disklens-core/tests/platform_paths.rs`（9 个用例，全部断言具体值）：

| 用例 | 断言 |
|---|---|
| `norm_path_str_unifies_separators_prefix_and_trailing_slash` | `C:\a\b\`→`C:/a/b`、`\\?\C:\a\b`→`C:/a/b`、`c:\A\B`→`C:/A/B`、`C:\`→`C:/`、`/`→`/`、`\\?\UNC\srv\share\x`→`//srv/share/x`、`""`→`""` |
| `windows_shaped_paths_hit_the_same_rules_as_linux` | `*/node_modules`→careful/前端依赖、`*/__pycache__`→safe/Python 缓存、`*/target/debug`→careful/构建产物；末尾分隔符、混合分隔符不改变结论，`rule_idx` 与 Linux 完全一致 |
| `windows_shaped_patterns_match_windows_shaped_paths` | 规则里写 `C:\Windows\*`、`C:\Program Files*`、`D:\Data\cache*` 都能命中；`C:/Windows/System32`（混合分隔符）也命中 |
| `guard_rejects_roots_system_dirs_and_lowercase_variants` | `/`、`C:\`、`C:/`、`\\?\C:\`→`系统根目录受保护`；`C:\Windows`→`Windows 系统目录受保护`；`C:\Windows\System32`、`c:\windows\system32`、`C:\Program Files\App\a.exe`→`位于受保护路径内` |
| `guard_rejects_traversal_and_expands_tilde` | `..`、`C:\Users\z\..\Windows`→`路径包含 ..（禁止越界）`；`~`→`主目录不能删除`；`~/Downloads/a.mkv`→放行 |
| `trailing_separator_does_not_bypass_guard` | `/usr/etc/` 与 `/usr/etc` 结果相同；`C:\Windows\System32\` 与不带尾部 `/` 相同 |
| `linux_rule_levels_are_unchanged` | 11 条代表性路径的 level+category（含石墙、系统、构建产物、venv、下载、临时）+ 规则条数 77 |
| `linux_matching_stays_case_sensitive` / `case_insensitive_platforms_fold_case_when_matching` | Linux 区分大小写（`/HOME/U/.SSH`→unknown）；macOS/Windows 折叠（→keep） |
| `scan_stats_report_hardlink_dedup_capability` | `stats.hardlink_dedup == cfg!(unix)` |

同时把会在 Windows/macOS 上**跑不过或编不过**的既有测试改对（只改门控与阈值，没删断言）：
- `rules_tests::default_table_is_ported_completely`：首条规则按平台分别断言（`C:\Windows` / `/System` / `/usr`），`exact_only` 计数 8（Linux）/9（Windows、macOS）；性能阈值 200ms（unix）/600ms（Windows，debug 下 fs+正则慢 2–3 倍，断言目标仍是复杂度）。
- `scan_fixture`：`std::fs` / `std::path::Path` 改成 `#[cfg(unix)]` 导入（只在 unix 用例里用）；`deep_40_levels` → `deep_24_levels`（`const DEPTH`，40 层 ≈330 字符超过 Windows 传统 MAX_PATH 260，真机会创建失败；24 层仍远超日常深度）。

## ② 验证证据

```
$ cargo test --workspace
running 9 tests   test result: ok. 9 passed   (disklens_lib，含 3 个 open.rs argv 用例)
running 1 test    test result: ok. 1 passed   (tests/ipc.rs)
running 10 tests  test result: ok. 10 passed  (guard_tests)
running 3 tests   test result: ok. 3 passed   (invariants)
running 9 tests   test result: ok. 9 passed   (ops_tests)
running 9 tests   test result: ok. 9 passed   (platform_paths，新增)
running 11 tests  test result: ok. 11 passed  (query_tests)
running 15 tests  test result: ok. 15 passed  (rules_tests)
running 20 tests  test result: ok. 20 passed  (scan_fixture)
running 9 tests   test result: ok. 9 passed   (snapshot_tests)
合计 96 passed / 0 failed（改前 84，新增 3 个 argv + 9 个跨平台用例）

$ cargo clippy --workspace 2>&1 | tail -3
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.13s
（无 warning / 无 error）
```

**Linux 分类结果与改前逐字节一致**（用 `git worktree` 检出 HEAD 跑同一棵树，只过滤耗时行）：

```
$ diff <(grep -v "耗时\|用时" /tmp/base2.txt) <(grep -v "耗时\|用时" /tmp/new3.txt) && echo IDENTICAL
IDENTICAL
```

```
🟢 安全清理         171.98 MiB
🟡 谨慎            14.54 GiB
❓ 待人工判断            0.00 B
🔴 不要动           17.56 GiB
🟢 可回收 171.98 MiB ｜ 🟡 谨慎重建 14.54 GiB
```
`safe 171.98 MiB` 与会话开始时完全一致。`careful` 从不含我的构建产物时的 11.48 GiB 涨到 14.54 GiB，原因是 `target/debug` 被本轮的 `cargo test --workspace` 从 9.41 GiB 撑到 12.47 GiB（HEAD 版本现在跑出同样的数字，diff IDENTICAL 已证明这不是分级行为变化）。

**Windows / macOS 交叉编译自证**（rustup 装 `x86_64-pc-windows-msvc`、`x86_64-apple-darwin`，`~/.cargo/bin` 未改 PATH）：

```
$ ~/.cargo/bin/cargo check -p disklens-core --target x86_64-pc-windows-msvc
    Checking disklens-core v0.1.0 (/home/zys/Projects/disklens/crates/disklens-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.32s     # 首次（修复 E0658 之前这里是 error）

$ ~/.cargo/bin/cargo check -p disklens-core --tests --target x86_64-pc-windows-msvc
    Checking disklens-core v0.1.0 (/home/zys/Projects/disklens/crates/disklens-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.14s

$ ~/.cargo/bin/cargo check -p disklens-core --tests --target x86_64-apple-darwin
    Checking disklens-core v0.1.0 (/home/zys/Projects/disklens/crates/disklens-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.14s
```
两个目标、lib 与 `--tests` 全部零 warning。**以后每次改 core 都该跑这两条**（约 10s，target 目录各占 314M / 50M，会算进 `target/`）。

## ③ 只能在 CI / 真机验证的部分

1. **真实 Windows / macOS 运行结果**：三平台 argv 只是字符串断言，`explorer /select,` 与 `open -R` 的真实行为、`%LOCALAPPDATA%` / `%SystemRoot%` 的实际展开值都还没在真机跑过。
2. **`src-tauri` 全量 Windows 编译**：`cargo check -p disklens --target x86_64-pc-windows-msvc` 卡在 `src-tauri/icons/icon.ico` 缺失（`required for generating a Windows Resource file during tauri-build`）—— 属 app 资源，不在本任务的所有权范围内，需要补图标或由 CI 的 Windows job 提供。
3. **macOS 规则表运行时表现**：`/Library/Caches/*` 与 `/Library/*`、`~/Library/*` 的优先级只做了静态推演（`cfg(target_os="macos")` 在本机不编译进去），要在 macOS CI 上用 `default_rules` 的条数与实际 classify 结果复核。
4. **Windows 硬链接不去重**：pnpm store / WinSxS 这类大量硬链接的场景，扫描大小会虚高（`stats.hardlink_dedup=false` 已明示），需要真机量化误差。
5. **Windows 上 `..`、符号链接、盘符根的 canonicalize 行为**：`\\?\` 前缀的真实产出格式、junction/symlink 解析只能真机验。
6. **性能阈值**：Windows 600ms / Linux 200ms 是估的（Linux 实测 117.6ms，HEAD 106.8ms，+10% 来自每节点一次归一化字符串分配），要在 Windows CI 上取首次真实数字。

## 诚实备注：关于「Windows 上分级静默失效」的根因

- globset 0.4 在**非 unix 目标**上自己就会把候选路径和模式里的 `\` 换成 `/`（`pathutil::normalize_path` + `is_separator('\\')`），所以「`*/node_modules` 匹配不到 `C:\...\node_modules`」这一现象在**真实 Windows 上未必复现**；任务里贴的三行实测是**在 Linux 上**跑出来的，而 Linux 分支不做这个替换 —— 那部分证据属于测试脚手架的形状不对称，不是 Windows 运行时的既有行为。
- **真正被实测钉死的 Windows 问题是编译失败**（E0658 `windows_by_handle`），已在 ①.5 修复并交叉编译验证。
- 即便如此，本次改动仍然有意义，因为它把三件事从「依赖 globset 平台分支的隐式行为」变成「自己显式、可跨平台断言」：分隔符归一化、`\\?\`/盘符/尾部分隔符、macOS 的大小写不敏感（这条是**真 bug**：macOS 默认文件系统不区分大小写，而 globset 默认区分，`~/projects` 不会命中 `~/Projects`）。另外护栏的 `..`、`~`、盘符根、尾部分隔符绕过现在都有测试兜底。

## 环境说明

- 为做交叉检查在本机装了 rustup（`~/.rustup`、`~/.cargo/bin`，约 400MB）与两个 target；`--no-modify-path`，没有改任何 shell 配置，`~/.cargo/bin` 不在 PATH 里。`rustup component add clippy` 也装了（本机 `cargo clippy` 会走 rustup 代理）。
- 未触碰 `app/**`、`docs/**`（除本报告）、`src-tauri/src/{commands,state,lib,main}.rs`。
