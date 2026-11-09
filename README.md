# DiskLens / 磁盘透镜

跨平台（Windows / macOS / Linux）磁盘空间分析工具：几秒扫完一个分区，用你一眼就懂的方式告出"是谁占了我的空间"，并能安全地下手清理。

- **Rust 扫描引擎**：单线程 `std::fs` 遍历，`~` 71.7 万条目 / 48.7 GB → **1.37 s**，峰值 RSS **97.7 MB**，与 `du --apparent-size` 误差 **0.002%**
- **Tauri 2 + React**：安装包约 10 MB 级，UI 用系统 WebView
- **不做整树 IPC**：树常驻 Rust arena，前端按父节点分页拉取，10 万节点照样 60fps
- **安全优先**：默认只读、删除默认进回收站、系统/代码目录在 Rust 层硬性拒绝

## 功能

| 视图 | 说明 |
|---|---|
| 目录树 | 虚拟滚动、四种排序、占比条、多选、双击下钻 |
| Treemap | Canvas 渲染，面积 = 大小，点击下钻并与树/面包屑联动 |
| 旭日图 | 径向层级分布，点击扇区下钻 |
| 类型分布 | 按扩展名 Top 12 聚合 |
| Top 大文件 | 50 / 100 / 500 条，点击定位到树 |
| 右键菜单 | 打开文件、在文件管理器中显示、复制路径/名称 |
| 清理 | 多选 → 回收站（默认）；永久删除需输入目录名二次确认 |
| 快照对比 | 保存扫描快照，对比两次找出"谁涨了" |

## 开发

```bash
# 前端（Mock 后端，浏览器直接可跑，无需 Rust）
cd app && npm ci && npm run dev          # http://localhost:5173

# 整个应用（必须在仓库根调用：tauri-cli 只在含 tauri.conf.json 的目录工作）
./app/node_modules/.bin/tauri dev

# 构建当前平台制品
./app/node_modules/.bin/tauri build      # 产物在 target/release/bundle/

# 测试
cargo test --workspace                   # Rust 核心 67 个测试
cd app && npx vitest run                  # 前端 38 个测试

# 性能复测
cargo run --release --example scan -- ~
```

## 平台构建

本机只构建当前平台。三平台制品由 GitHub Actions 矩阵构建（`.github/workflows/ci.yml`）：

| 平台 | 产物 |
|---|---|
| Ubuntu 22.04 | `.deb` / `.AppImage` |
| Windows | `.msi` / `.exe`（NSIS） |
| macOS (arm64) | `.dmg` / `.app`（未签名） |

产物在 Actions 运行页的 Artifacts 下载，保留 30 天。

## 文档

- [产品需求 PRD](docs/01-产品需求文档-PRD.md)
- [技术选型](docs/02-技术选型.md)
- [架构设计](docs/03-架构设计.md)（含 IPC 契约）
- [测试计划](docs/04-测试计划.md)
- [实施计划](docs/05-实施计划.md)
- [跨平台支持与验证矩阵](docs/06-跨平台支持.md)（哪些平台真跑过、哪些差距在哪）
- [性能基准](docs/bench.md)

## 已知限制

- 大小口径为**逻辑大小**（`st_size`），与 `du` 默认的块占用口径相差约 3%（稀疏/取整），属预期。
- `/proc`、`/sys` 内核伪文件系统无意义，默认排除。
- 稀疏文件、APFS 克隆、Windows 压缩文件按逻辑大小计。
- **Windows 下硬链接不去重**（稳定版 Rust 的 std 无法读取 file_index/nlink）：pnpm store、WinSxS 等场景占用会虚高；Linux/macOS 按 `(dev, ino)` 精确去重。
- 未签名 macOS 包首次打开需 `xattr -dr com.apple.quarantine /Applications/DiskLens.app`。
