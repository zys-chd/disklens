# T5 — 安全分级规则引擎（safe / careful / keep / unknown）

> 2026-11-10 ｜ crate：`crates/disklens-core` ｜ 验证机：Arch Linux，rustc/cargo 1.99.1
> 依赖新增：`globset 0.4.21`（通配匹配，替代 Python `fnmatch`）

## ① 做了什么

### 新增 / 修改文件

| 文件 | 内容 |
|---|---|
| `src/rules.rs`（新） | `SafetyLevel` / `Safety` / `Rule` / `RuleSet` / `SafetyStat` / `SafetyReport`；默认规则表；`classify_tree` / `safety_stats` / `default_rules_path` / `RuleSet::load_or_create` |
| `tests/rules_tests.rs`（新） | 15 个测试：继承/顺序/石墙/basename/规则文件/统计/5 万节点性能 |
| `examples/classify.rs`（新） | 扫 `~/Projects`，打印各等级汇总 + Top 10 可清理目录 |
| `src/model.rs` | `Tree` 增加平行数组 `pub safety: Vec<Safety>`（索引 = NodeId，Node 布局不变） |
| `src/scan.rs` | 扫描产物 `safety: Vec::new()`（`classify_tree` 前为空） |
| `src/lib.rs` | `pub mod rules` + 统一导出 |
| `tests/query_tests.rs` | 合成 Tree 补 `safety: Vec::new()` |
| 根 `Cargo.toml` / `Cargo.lock` | 加 `globset`；`[profile.dev.package.globset|regex-automata] opt-level=3`，让 debug 性能守卫有可比性 |

### 核心语义（逐条对齐旧 `rules.py`）

- **等级**：`safe` 🟢 / `careful` 🟡 / `keep` 🔴 / `unknown` ❓；`SafetyLevel::order()` 保持旧版 `safe0 < careful1 < unknown2 < keep3`。
- **胜出规则**：规则列表中最先命中「路径自身或其任一祖先」的一条。顺序即优先级。
- **`exact_only`**：只命中自身，不向后代继承（`/usr`、`/etc`、`/boot`、`/opt`、`/var/tmp`、`~`、`/home`、`/`）。
- **增量收敛**：`scan_one(path, bound)` 只在 `[0, bound)` 查；子节点的 `bound = min(自身可继承命中, 父 bound)`，单调递减，越深越快。
- **双匹配**：同一模式同时试完整路径与 basename（旧版 `_hit` 行为）；basename 只对无 `/` 的模式建独立 `GlobSet`。
- **fnmatch 语义**：`GlobBuilder::literal_separator(false)` 让 `*` 也跨越 `/`（`/var/log/*` 命中 `/var/log/nginx/access.log`），与旧版一致；`?` `**` `[abc]` 由 globset 支持。
- **`~` 展开**：用调用方传入的 `home`，不读进程环境，测试可注入假 home。
- **规则文件**：`dirs::config_dir()/disklens/rules.json`；不存在→自动创建默认规则；JSON 损坏/空 `rules`→回退默认且不覆盖坏文件；未知 level 字符串→`unknown`；非法 glob 退化为字面量匹配，绝不 panic。

### 默认规则表

旧版 `DEFAULT_RULES` **77 条全部移植，顺序一一对应**（脚本逐条比对：`missing: []`、`order == True`），字段完整保留 `pattern/category/level/reason/method/needs_root/suggest_cmd/exact_only`。

平台追加（运行时 `cfg!` 判断，规则文件按平台生成）：

- Windows（+6，共 83 条）：`C:\Windows`（exact）、`C:\Windows\*`、`C:\Program Files*`、`C:\ProgramData\*` 为 keep；`%LOCALAPPDATA%\Temp\*`、`%LOCALAPPDATA%\Microsoft\Windows\INetCache\*` 为 safe；`%VAR%` 运行期展开。
- macOS（+7，共 84 条）：`/System`（exact）、`/System/*`、`/Library/*` keep；`~/Library/Caches/*`、`~/Library/Logs/*` safe；`~/Library/Developer/Xcode/DerivedData/*`、`*.dSYM` careful。

### 统计 API

```rust
pub fn classify_tree(tree: &mut Tree, rs: &RuleSet)
pub fn safety_stats(tree: &Tree, rs: &RuleSet) -> SafetyReport
// SafetyReport { stats: Vec<SafetyStat>, safe_bytes, careful_bytes }
// SafetyStat  { category, level, bytes, files, count }
```

- 只统计 `NodeKind::Dir`；同 `category+level` 的父子目录只计**最上层**一次（`bytes/files/count` 不重复）。
- `safe_bytes` / `careful_bytes` 另按「父目录等级相同则跳过」聚合，嵌套同等级不会双计。
- 按 bytes 降序，同值按等级、category 兜底保证确定性。

## ② 验证证据

### `cargo test --workspace`（真实输出）

```
Running unittests src/lib.rs (disklens_lib)        4 passed
Running unittests src/main.rs (disklens)           0 passed
Running tests/ipc.rs                               1 passed
Running unittests src/lib.rs (disklens_core)       0 passed
Running tests/guard_tests.rs                      10 passed
Running tests/invariants.rs                        3 passed
Running tests/ops_tests.rs                         9 passed
Running tests/query_tests.rs                      11 passed
Running tests/rules_tests.rs                      15 passed
Running tests/scan_fixture.rs                     20 passed
Running tests/snapshot_tests.rs                    9 passed
Doc-tests                                          0 passed
test result: ok. 全部 0 failed
```

合计 **82 passed / 0 failed**（原有 67 + 新增 15）。`rules_tests` 15 条：

```
default_table_is_ported_completely ... ok
node_modules_careful_is_inherited_by_deep_file ... ok
specific_cache_rule_beats_generic_cache_fallback ... ok
user_config_kept_and_projects_build_artifacts_win_on_order ... ok
system_rule_keeps_site_packages_under_usr ... ok
slash_star_crosses_separators_like_fnmatch ... ok
pycache_matches_by_basename_anywhere ... ok
home_wall_is_exact_only_and_not_inherited_by_downloads ... ok
rules_file_created_on_first_run_and_reloadable ... ok
corrupt_rules_file_falls_back_to_default_without_panic ... ok
unknown_level_string_maps_to_unknown ... ok
safety_stats_counts_nested_same_group_once ... ok
scan_then_classify_fills_parallel_array ... ok
classify_and_stats_50k_nodes_under_200ms ... ok
classify_absolute_and_tilde_paths_agree ... ok
```

关键断言（具体值）：

| 场景 | 断言 |
|---|---|
| `app/node_modules/foo/x.js` | `Careful` / `前端依赖` / `rule_idx == 祖先 */node_modules` |
| `~/.cache/video-upscaler/tmp/a.bin` | `Safe` / `应用缓存`，索引 < `~/.cache/*` |
| `~/.config/x` | `Keep` / `配置` |
| `~/Projects/a/target/debug/x` | `Careful` / `构建产物`，`~/Projects/a/target` 仍为 `Keep` |
| `/usr/lib/python3.14/site-packages/x` | `Keep` / `系统`，`rule_idx == /usr/*` |
| `/mnt/data/venv/lib/site-packages/x` | `Careful` / `Python 虚拟环境`（对照） |
| `/var/log/nginx/access.log` | `Careful` / `系统日志`（`*` 跨 `/`） |
| `/mnt/data/proj/__pycache__/m.pyc` | `Safe` / `Python 缓存` |
| `~` / `~/Downloads/foo` | `Keep` 石墙 / `Unknown` 下载（石墙不继承） |
| 统计树 950B 根 + 600B node_modules/foo + 300B Downloads + 50B pycache | `safe=50`、`careful=600`（不是 1200），`careful.count=1`，4 行分组 |
| 规则文件不存在 | 自动创建、JSON 可解析、`rules.len()==77`、可重载并保持分类 |
| JSON 损坏 / level=`SuperSafe` | 回退 77 条默认 / 单条规则映射为 `Unknown`，均不 panic |

### 性能守卫（5 万节点，debug 测试）

```
classify+stats 50001 节点耗时 121.159884ms（safe 0 B, careful 49664000 B）
test result: ok. 1 passed; 0 failed; ... finished in 0.12s
```

断言 `< 200ms`。合成树为 500 个项目 × `target/debug/f0..f96.rs`，即真实 `~/Projects` 形态。

### `examples/classify.rs` 实测（扫 `~/Projects`）

```
扫描 /home/zys/Projects …
规则 77 条（/home/zys/.config/disklens/rules.json），扫描节点 66620，用时 175 ms
分类+统计耗时 147.572621ms

== 各等级（按 category+level 分组，嵌套同组只计一次）==
🟢 安全清理         171.98 MiB
🟡 谨慎            11.26 GiB
❓ 待人工判断            0.00 B
🔴 不要动           14.02 GiB

🟢 可回收 171.98 MiB ｜ 🟡 谨慎重建 11.26 GiB

== Top 8 分类分组 ==
用户代码         🔴 不要动           14.02 GiB        1 个目录
构建产物         🟡 谨慎             9.32 GiB        3 个目录
Python 虚拟环境  🟡 谨慎             1.15 GiB        5 个目录
前端依赖         🟡 谨慎           810.48 MiB        2 个目录
Python 缓存    🟢 安全清理         171.98 MiB     1288 个目录

== Top 10 可清理目录（safe/careful，无需 root）==
  9.20 GiB  🟡 谨慎       /home/zys/Projects/disklens/target/debug
            └ Rust 调试产物（cargo clean 更安全）（cargo clean）
665.17 MiB  🟡 谨慎       /home/zys/Projects/icloud-reminders/node_modules
            └ npm 依赖，删除后需重新 npm install（确认可重装后再删）
649.48 MiB  🟡 谨慎       /home/zys/Projects/video-upscaler/.venv
            └ 项目虚拟环境（确认不用后再删）
289.03 MiB  🟡 谨慎       /home/zys/Projects/reliability-lab-system/backend/.venv
            └ 项目虚拟环境（确认不用后再删）
165.16 MiB  🟡 谨慎       /home/zys/Projects/fund-dca/.venv
            └ 项目虚拟环境（确认不用后再删）
145.32 MiB  🟡 谨慎       /home/zys/Projects/disklens/app/node_modules
            └ npm 依赖，删除后需重新 npm install（确认可重装后再删）
128.73 MiB  🟡 谨慎       /home/zys/Projects/disklens/target/release/build
            └ 构建输出，可重新构建（按需清理）
 61.65 MiB  🟡 谨慎       /home/zys/Projects/icloud-reminders/.venv
            └ 项目虚拟环境（确认不用后再删）
 10.49 MiB  🟡 谨慎       /home/zys/Projects/disk-analyzer/.venv
            └ 项目虚拟环境（确认不用后再删）
  5.34 MiB  🟢 安全清理     .../site-packages/plotly/graph_objs/__pycache__
            └ 字节码缓存，自动重新生成（可批量删除）
```

首次运行生成的规则文件（`~/.config/disklens/rules.json`，77 条逐条一行）：

```json
{
 "version": 1,
 "rules": [
  {"pattern":"/usr","category":"系统","level":"keep","reason":"系统文件","method":"不要动","needs_root":false,"suggest_cmd":"","exact_only":true},
  ...
 ]
}
```

## ③ 已知缺口 / shortcut

1. **相对模式不拼 home**（与任务描述“相对模式用 home 展开”不同）：`*/node_modules`、`*/site-packages` 保持相对，按 fnmatch 匹配任意位置。若拼 home，`/tmp`、外接盘上的同名目录会漏判；以旧 Python `_norm`（仅 `~`/`/` 开头展开）为准，代码内有注释。
2. **`safety_stats` 返回 `SafetyReport` 而非裸 `Vec`**：任务要求“额外返回汇总”，合并成一个结构体；`report.stats` 即原来的 Vec。
3. **树根之上的祖先不参与分类**：`classify_tree` 只从扫描根开始（旧版 `apply_rules` 同）。直接扫 `~/.cache` 时根自身按 `~/.*` 判 keep、子目录 safe，属旧语义。
4. **跨分组字节有重叠**：`keep 用户代码 14GiB` 包含其下 careful 的 `target`/`node_modules`；这是“分组汇总”而非“互斥分区”。`safe_bytes`/`careful_bytes` 只在同等级嵌套间去重。
5. **Windows/macOS 规则未实机验证**：Linux 仅验证 77 条；Windows 路径分隔符（统一转 `/`）与 `%VAR%` 展开、macOS 的 `~/Library/*` 均无真机测试。另：旧版 `fnmatch` 在 Windows 上大小写不敏感，本实现统一大小写敏感（Linux 语义），Windows 大小写混写的路径可能漏判。
6. **规则热更新**：`RuleSet` 在加载时编译 glob；改 JSON 后需重新 `load_or_create`，无文件监听。
7. **性能守卫是 debug 计时**：靠 `[profile.dev.package.*] opt-level=3` 让 globset/regex 接近 release；本机 5 万节点 110–121ms、6.6 万节点约 148ms，仍有约 1.6× 余量，机器极慢时可能擦线。
8. **未接 IPC/前端**：`Safety`/`SafetyStat` 尚未进 `NodeView` 或 Tauri 命令（属 T2/T3 范围）；`Tree.safety` 为空时 `safety_stats` 只有 `debug_assert`，release 下会把所有目录当 `unknown` 统计。
9. **规则文件路径变更**：旧版 `~/.config/disk-analyzer/rules.json` 不自动迁移到 `~/.config/disklens/rules.json`（按要求使用新路径）。
10. **`Glob` 非法时退化为字面量**：不会 panic，但该规则从此只匹配字面字符串；用户写坏 glob 无提示。
