# T6 — 安全分级接入 Tauri IPC 与 React 界面

## ① 做了什么

### Rust（`src-tauri/**`）

| 改动 | 位置 |
|---|---|
| `ScanRecord` 新增 `rules: RwLock<Option<Arc<RuleSet>>>`，`AppState::rules(id)` 取回；规则表与树同生命周期（最近 3 次扫描） | `state.rs` |
| `start_scan` 扫描线程：扫描成功 → `RuleSet::load_or_create(default_rules_path())` → `classify_tree` → **先落 rules 再落 tree** → 发 `scan://done`。规则文件缺失/损坏由 `load_or_create` 回退默认，不阻断扫描、不报错 | `commands.rs` |
| `NodeView` 增加 6 个可选字段：`safety`、`safetyCategory`、`safetyReason`、`safetyMethod`、`safetyNeedsRoot`、`safetyCommand`。全部 `skip_serializing_if = None`：未分类的树序列化结果与旧契约逐字一致 | `commands.rs` |
| `list_children` 增加可选 `safety?: string[]` / `maxLevel?: string`；过滤在 Rust 侧完成。有过滤时先取全量（≤`MAX_LIMIT`）、过滤后再分页，`total` 是过滤后的真实数 | `commands.rs` |
| 新命令 `safety_stats({scanId})` → `SafetyReport`（`stats`/`safeBytes`/`carefulBytes`，camelCase） | `commands.rs` |
| 新命令 `open_rules_file()`：`rules.json` 不存在先用 `load_or_create` 写出默认规则，再交给既有 `open::open_path` | `commands.rs` |
| 命令注册 | `lib.rs` |
| 单测：安全过滤语义（allow/max 组合）、分类后 NodeView 字段、未分类树字段不变 | `commands.rs` |
| IPC 集成测试补 `safety` 字段 / `safety_stats` / `safety:["safe"]` 过滤断言；并设 `XDG_CONFIG_HOME` 到临时目录，避免扫描测试写脏真实 `~/.config` | `tests/ipc.rs` |

### 前端（`app/**`）

- **契约**：`types.ts` 增 `SafetyLevel`、NodeView 可选分级字段、`SafetyStat`/`SafetyReport`、`ListChildrenArgs.safety/maxLevel`、`Backend.safetyStats/openRulesFile`；`tauri.ts` 接两条新命令。
- **树行**：名称前 2px 圆点（safe 绿 / careful 黄 / keep 红 / unknown 灰），圆点与行 `title` 都显示 `category · reason`；`keep` 行名称 `opacity-60`。
- **汇总条** `components/SafetyBar.tsx`（树上方）：`🟢 可安全清理 X  🟡 谨慎重建 Y  [只看可清理] [规则]`；「规则」调 `open_rules_file`。
- **过滤**：store 增 `onlySafe` / `safety` / `setOnlySafe` / `loadSafety`；`childKey` 计入 `onlySafe`（缓存键），`ensureChildren` 传 `safety: ['safe']`；`scan://done` 后与 top/type 并行加载 `safetyStats`。
- **删除对话框**：选中项含 careful/keep 时警示并逐条列出（keep 另标「后端会直接拒绝」）；有 `safetyCommand` 时显示「建议命令」块，只复制、不执行；`safetyNeedsRoot` 提示 sudo。
- **右键菜单**：节点带 `safetyCommand` 时多一项「复制建议命令」。
- **纯函数** `lib/safety.ts`：`SAFETY_DOT` / `SAFETY_LABEL` / `safetyDot` / `safetyTitle` / `safetySummary` / `riskyItems`。
- **Mock**：按 core 默认规则顺序的极简分类器（祖先继承，node_modules/构建产物 → careful，.cache → safe，Projects/Steam/.config/.ssh/.hermes → keep，Downloads → unknown）；构造时一次算完；`view()` 带全部 safety 字段；`safetyStats` 按 category+level 聚合且嵌套同组只计最上层；`listChildren` 支持 `safety`/`maxLevel`；`openRulesFile` 为 no-op。

### 测试

- Rust：82 → **84**（新增 2 个单元测试；ipc 集成测试内补断言）。
- 前端：38 → **48**：`safety.test.ts`（圆点映射含缺失值、汇总条 0 字节/无报告 `—`、tooltip 拼接）、`deleteDialog.test.tsx` +3（careful/keep 警示与被拦数、全 safe 不提示、建议命令显示）、`mock.test.ts` +2（汇总聚合、safety 过滤）。

## ② 验证证据

```
$ cd /home/zys/Projects/disklens && cargo test --workspace
TOTAL_RUST_TESTS=84            # 13 个 test binary 全部 ok，0 failed

$ cargo clippy -p disklens 2>&1 | tail -3
    Checking disklens v0.1.0 (/home/zys/Projects/disklens/src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.49s
```

```
$ cd app && npx tsc --noEmit
TSC: ok

$ npx vitest run
 Test Files  7 passed (7)
      Tests  48 passed (48)

$ npm run build
dist/assets/index-CVe2fHtx.js   358.07 kB │ gzip: 112.37 kB
✓ built in 784ms
```

```
$ ./app/node_modules/.bin/tauri build --no-bundle        # 仓库根调用
   Compiling disklens v0.1.0 (/home/zys/Projects/disklens/src-tauri)
    Finished `release` profile [optimized] target(s) in 14.40s
       Built application at: /home/zys/Projects/disklens/target/release/disklens
```

真机（GNOME 拒绝截图，只验存活/无 stderr）：

```
$ pkill -x disklens
$ XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0 target/release/disklens &
$ sleep 8; pgrep -x disklens
106529   ALIVE        # 进程存活 12s+，stdout/stderr 为空
```

浏览器（mock 后端，`vite --port 5173`）逐项实测：

```
汇总条:  可安全清理 5.3 GB  谨慎重建 6.4 GB  只看可清理  规则
根行色点: 13 个 [aria-label]，keep 行名称 opacity-60（.local/Videos/Projects/Music/Pictures/Documents/.hermes/.config/.ssh/…）
行 tooltip: ".local\n46.8 GB · 455 个文件\n用户数据 · 本地用户数据"
只看可清理 ON: 行数 13 → 1（只剩 .cache，圆点全是 safe）；OFF 恢复 13
右键 pip: "pip | 打开文件 | 在文件管理器中显示 | 复制路径 | 复制名称 | 复制建议命令"
删除 .cache/electron: "共 1 项 … ⚠ 1 项不是安全级，删除前请确认 | electron | Electron 二进制缓存"
删除 .cache/pip: "建议命令（复制后自行在终端执行） | pip cache purge | 复制"
```

## ③ 缺口 / 已知限制

1. **过滤上限**：`list_children` 的安全过滤在命令层做（core 的 `query.rs` 不认识分级，且本次不允许改）。单目录超过 5000 个子项时，「只看可清理」只覆盖前 5000 条；根治需把 `safety` 过滤下放到 `ChildQuery`。已按代码注释约定标 shortcut。
2. **规则热重载**：分类在扫描线程一次性完成，改完 `rules.json` 需要重新扫描才生效；`open_rules_file` 打开后没有「重新分类」入口。
3. **`safety_stats` 前提**：core 里依赖「先 `classify_tree`」的 debug 断言；当前只有 `start_scan` 会产树，所以安全，但任何新增的非扫描写入路径都必须记得分类。
4. **分级字段粒度**：按需求「可忽略文件」，实现上对所有节点都填了（文件继承父目录等级）。若嫌文件行圆点噪音，在 `commands.rs::view` 加 `n.kind == Dir` 判断即可。
5. **`stats` 数组未上 UI**：IPC 已返回按 category+level 的明细，界面只用 `safeBytes`/`carefulBytes` 与「只看可清理」。按需再加展开面板。
6. **真机肉眼未验**：GNOME 拒绝截图，WebKit 侧只验证进程不崩；所有 UI 行为均在浏览器 mock 上确认（同一份 React 代码，`isTauri()` 分支只差 backend 实现）。
7. 测试副作用：`ipc.rs` 现在把 `XDG_CONFIG_HOME` 指到临时目录；此前跑测试会写真实 `~/.config/disklens/rules.json`（该文件现已存在，内容为默认规则，删除无副作用）。
