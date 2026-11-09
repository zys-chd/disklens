# T3 前端交付报告（app/ + Mock 后端）

> 任务：DiskLens 前端，React 19 + TS strict + Vite + Tailwind 4 + zustand + TanStack Virtual + d3。
> 范围：仅 `app/**`；文档只写本报告。真实 Tauri 后端未参与，浏览器内使用 Mock。

---

## 1. 做了什么

### 1.1 技术栈（实际安装版本）

`react@19.3.1`、`react-dom@19.3.1`、`vite@7.3.8`、`typescript@5.9.3`、`tailwindcss@4.3.5`（`@tailwindcss/vite`）、`zustand@5.1.0`、`@tanstack/react-virtual@3.14.26`、`d3-hierarchy@3.1.3`、`d3-shape@3.2.1`、`vitest@3.2.7`、`jsdom@26.1.0`、`@testing-library/react@16.3.3`、`@tauri-apps/api@2.12.4`。
无 UI 组件库、无图表库、无 router、无动画库。

### 1.2 结构与契约

- `src/lib/types.ts`：逐字镜像架构文档 §4（`NodeView` / `Drive` / `ScanSummary` / `OpResult` / 事件 payload / `Backend` 接口）。
- `src/lib/backend/index.ts`：`isTauri()` 判断 `window.__TAURI_INTERNALS__`，选择 `tauriBackend` 或 `mockBackend`。
- `src/lib/backend/tauri.ts`：按契约命令名（snake_case）+ camelCase 参数调用 `invoke`，`scan://progress|done|error` 用 `listen`。
- `src/lib/backend/mock.ts`：确定性假树，固定 seed，**精确 20000 个节点**，幂律文件大小；含 `node_modules`（300 个包）、`.cache`、Steam 游戏（Cyberpunk 2077 / Elden Ring 等）、`large.bin`、iso、视频、缩略图长尾。实现 `Backend` 全部方法：分页/排序（size/name/files/mtime，稳定 id tie-break）/大小写不敏感名称过滤/minSize、top_files、type_stats、回收站与永久删除模拟（逐项 ok/failed、guard 拒绝、占用失败）、快照方法形状、进度事件与完成事件。扫描默认 2500ms，4 次以上进度 tick；`cancel` 80ms 后发 cancelled 的 done。
- `src/store/scanStore.ts`：zustand 管理 scanId/summary/进度/展开集/子节点缓存/排序/过滤/选中/面包屑链/对话框/toast；缓存 key = `nodeId:sortBy:desc:filter:minSize`，排序/过滤变化整体清空重载。
- `src/lib/tree.ts`：`flattenRows`、`sortNodes`、`compareNodes`、`childKey`；纯函数。
- `src/lib/format.ts`：`formatBytes`（1024 进制、1 位小数、1023.95 进位）、`formatMtime`、`formatCount`、`percent`。
- `src/lib/treemap.ts`：`toTreemapInput` + `d3.treemap` 布局，输出单层矩形。
- `src/lib/delete.ts`：`basename`、`confirmValid`（回收站免输入；永久删除必须 exact basename；多选永久删除拒绝）。

### 1.3 UI

- 顶栏：Drive 下拉（显示已用/总量）、目录输入、扫描、搜索（100ms 防抖）、全部/≥1MB/≥10MB/≥100MB 过滤、扫描进度行（已扫条目/字节/当前路径/耗时 + 停止）。
- 面包屑：根路径、祖先链、上一级/回根、不可读条目数徽标；点击回退。
- 目录树：TanStack Virtual（overscan 8、28px 行高）、展开/收起、名称/大小+占比条/文件数/时间四列、四字段升降序、Ctrl/Cmd 多选、Shift 连选、双击下钻、右键菜单。
- 右面板：Treemap（Canvas + d3.treemap，hover tooltip + 点击下钻）、旭日图（SVG + d3.partition + d3.arc，2 层环，点击下钻）、类型分布（Top 12 + 其他，横向条形）。
- 底部：Top 50/100/500 大文件，点击定位树并选中目标，行内打开/显示按钮。
- 选中浮动条：N 项 + 合计；移入回收站；永久删除（单选取可用）；清除选择。
- 右键菜单：打开文件 / 在文件管理器中显示 / 复制路径 / 复制名称；点击外部、Esc、滚动关闭。
- 删除对话框：回收站一次确认；永久删除必须输入 basename，实时校验；结果逐项 ✓/✗ + 错误原因；失败项可重试。
- 空目录、无匹配、扫描中骨架、错误态（扫描失败可重试）、Toast。
- 视觉：`#6366f1` 强调色，safe/warn/danger 语义色；系统 `prefers-color-scheme` 深浅色；等宽数字 `tabular-nums`；Linear/Raycast 式克制密度。

### 1.4 性能相关实现

- 树只渲染可视行（overscan 8）。
- Treemap 布局只在 `(focus, children, 容器尺寸, hovered)` 变化时重算；Canvas 按 DPR 绘制；布局 ≤ 5000 子项（后端 limit 5000）。
- 子节点单目录一次取 limit 5000（契约上限），按 key 缓存；重扫/排序/过滤清空，防跨 scan 污染。
- 旭日图仅预取 focus + 前 10 个目录，深度 2，避免 N 次全树请求。
- 搜索 100ms 防抖；Canvas hover 只重绘 hover id 变化。

---

## 2. 验证证据（真实输出）

### 2.1 安装 / 类型 / 构建 / 单测

命令：

```bash
cd /home/zys/Projects/disklens/app && npm install --no-audit && npx tsc --noEmit && npm run build && npx vitest run
```

输出（tsc 无输出 = 0 错误，随后构建成功）：

```text
added 3 packages in 26s

35 packages are looking for funding
  run `npm fund` for details

> disklens-app@0.1.0 build
> tsc --noEmit && vite build

vite v7.3.8 building client environment for production...
transforming...
✓ 69 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                   0.45 kB │ gzip:   0.29 kB
dist/assets/index-Bk243KxG.css   23.55 kB │ gzip:   5.37 kB
dist/assets/index-BQTLfdds.js   335.63 kB │ gzip: 105.83 kB
✓ built in 774ms

 RUN  v3.2.7 /home/zys/Projects/disklens/app

 ✓ src/delete.test.ts (4 tests) 2ms
 ✓ src/tree.test.ts (9 tests) 8ms
 ✓ src/treemap.test.ts (4 tests) 4ms
 ✓ src/format.test.ts (5 tests) 11ms
 ✓ src/deleteDialog.test.tsx (3 tests) 82ms
 ✓ src/mock.test.ts (13 tests) 223ms

 Test Files  6 passed (6)
      Tests  38 passed (38)
   Start at  00:10:41
   Duration  827ms (transform 197ms, setup 427ms, collect 407ms, tests 331ms, environment 1.38s, prepare 380ms)
```

覆盖的断言（均为具体值，不是 truthy）：

- `formatBytes`：0B、1023B、1024B→1.0 KB、1536→1.5 KB、1.5GB、1.0TB、1.5TB、1048575→1.0 MB、1073741823→1.0 GB。
- `flattenRows`：嵌套展开/收起后的行名与 depth 序列 `[A,A1,a.txt,b.txt,A2,B]` / `[0,1,2,2,1,0]`；无缓存时不下钻。
- 排序稳定：size/mtime/files/name，等值项保持插入顺序（id tie-break）。
- Mock 分页/排序：total=13；`offset 0 limit 3` + `offset 3 limit 4` 与全量前 7 条一致；name asc 首项 `.cache`；filter `DISKLENS`→1 项 `disklens`；minSize 结果与全量筛选一致；top file `large.bin` = 6GB+720MB；iso stat = 3GB+100MB / 1 个；`.pak` = 123 个。
- 删除：重复回收站→`文件不存在`；`/usr`→`系统路径受保护`；`/home/zys`→`受保护目录，禁止删除`；locked→`文件被占用（模拟）`；永久删除错误 confirmText→`确认文本与目录名不一致`。
- 扫描：4 次进度（5000 / … / 19400 项），done entries=20000；取消后 cancelled=true 且 entries<20000。
- Treemap：输入映射 3 个子项、面积和 1000；布局 3 个矩形、全在画布内、大/小面积比 >5。
- 删除对话框组件：初始 disabled、错误名称 disabled、正确名称 enabled；多选永久删除 disabled 并显示说明。

### 2.2 浏览器运行（Mock）

```bash
cd /home/zys/Projects/disklens/app && npm run dev -- --host 127.0.0.1
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:5173/
```

```text
200
LISTEN 0 511 127.0.0.1:5173 ... ("node",pid=47048)
```

真实浏览器（Chromium，1280×820）验证结果：

```text
初始扫描完成：13 个根行；Canvas 537×396；无横向溢出（scrollWidth=innerWidth=1280）
Treemap 不透明像素 75954、颜色采样 27 种；hover tooltip =
  ".local46.8 GB · 56.9%455 个文件 · 目录"
双击 Projects → 面包屑 ["/home/zys","Projects"]，树出现 disklens/reliability-tool 子行
点击 Treemap → 面包屑 ["/home/zys","Projects","disklens"]
旭日图：21 条 path（全部有 d），中心 "disklens / 4.2 GB"
类型分布：12 条 + 其他；Top3 = .pak 46.6 GB / .bin 7.6 GB / .mp4 7.1 GB
选择 Downloads → "已选 1 项 · 合计 13.0 GB"
永久删除对话框：输入框 placeholder=Downloads，初始 confirm disabled，输入 Downloads 后 enabled
回收站执行 → "成功 1 项 · 失败 0 项"，toast 正常
点击 Top 文件 large.bin → 面包屑 ["/home/zys","Downloads"]，树选中 large.bin
扫描→停止 → toast "扫描已取消，展示已扫到的部分"，行恢复
深色模式 computed：--bg #0e0f12，body rgb(14,15,18)，header rgb(21,22,26)
980×600：scrollWidth=980；header 44 + nav 32 + right 411.6 + bottom 228，无溢出
window.onerror / console.error 捕获：[]（空）
```

截图文件（浏览器执行时生成，仅本机路径）：
`/home/zys/.config/browser-harness/tmp/shot.png`。

验证完成后已停止 dev server，避免占用 5173；重新 `npm run dev` 即可复现。

---

## 3. 已知缺口 / shortcut

代码内均以 `shortcut:` 注释标注，主要：

1. **搜索不递归**（`components/TopBar.tsx`）：IPC `filter` 只过滤当前目录直属子项，根级搜索 `.pak` 会得到空结果。需要后端新增全局 search 命令后升级。
2. **永久删除仅单项**（`lib/delete.ts`）：Rust guard 要求 `confirmText` 等于每个目标 basename，一个输入框无法安全对应多个 basename。多选时按钮 disabled，对话框提示改用回收站。
3. **Mock 取消仍返回完整树**（`lib/backend/mock.ts`）：真实 core 返回已扫部分；Mock 用 `cancelled` 标记 + 部分 entries 模拟，查询仍是模板全树。
4. **Mock 删除不改树**：扫描结果按快照语义只读；删除只记录 simulated deleted path（重复删除报"文件不存在"），需重扫才刷新树。真实后端同样需要重扫，但 Mock 未模拟树缩减。
5. **快照**：`saveSnapshot/listSnapshots/diffSnapshots` 按契约返回形状（内存记录 / 空 diff），本版没有快照 UI（F16 P1，不在 T3 必做列表）。
6. **类型分布不可点击过滤**（`components/TypeStats.tsx`）：契约只有名称子串过滤，按扩展名过滤根目录会得到 0 行，故本版只展示 Top12+其他；需要后端 type filter / search 后升级。
7. **进度条无百分比**：IPC progress 无 total 字段，UI 用不定长条纹 + 已扫条目/字节/路径/耗时；需要契约增加 total 才能显示 62%。
8. **浏览器内 openPath 是 no-op**（`lib/backend/mock.ts` 仅 console），真实行为由 Tauri 实现。
9. **Treemap 单层**：每次只画当前节点下一层，点击下钻；没有嵌套小矩形/动画。
10. **无拖拽 / 原生目录选择器**：本版用顶栏目录输入；F1 的拖入/选择器需接 Tauri dialog 插件。
11. Top 大文件最多 500 行，未虚拟滚动（500 DOM 可接受）。
12. Mock `platformInfo` 固定 Linux、路径固定 `/home/zys`；真实平台信息由 Rust 返回。
13. 无 CSV 导出（F17 P2）、无多语言（PRD 明确不做）、无快照页面。
14. 未做键盘树导航（仅 Enter 选中、Tab 到行）；可访问性为基本 aria/title。
15. `i18n/zh.ts` 只保留了基础字典，组件内仍有内联中文；PRD 明确不做多语言，若后续要加语言需逐组件替换。

---

## 4. 文件清单（仅 app/，另加本报告）

```
app/package.json, package-lock.json, tsconfig.json, vite.config.ts, index.html, .gitignore
app/src/main.tsx, App.tsx, index.css
app/src/i18n/zh.ts
app/src/lib/{types,format,tree,treemap,delete}.ts
app/src/lib/backend/{index,tauri,mock}.ts
app/src/store/scanStore.ts
app/src/components/{TopBar,Breadcrumb,ScanProgress,TreeView,TreemapCanvas,Sunburst,TypeStats,
  BottomPanel,SelectionBar,ContextMenu,DeleteDialog,Toast,ui,icons,useBox}.tsx|ts
app/src/{format,tree,treemap,delete,mock}.test.ts
app/src/deleteDialog.test.tsx, src/test/setup.ts
docs/reports/T3-frontend.md（本文件）
```

运行：

```bash
cd /home/zys/Projects/disklens/app
npm install
npm run dev        # 浏览器打开 http://localhost:5173
npm run build
npx vitest run
```
