import type {
  Backend,
  DiffResult,
  Drive,
  ListChildrenArgs,
  NodeKind,
  NodeView,
  OpResult,
  PlatformInfo,
  ScanOptions,
  ScanProgress,
  ScanSummary,
  SnapshotInfo,
  SortBy,
  TopFile,
  TypeStat,
} from '../types'

const KB = 1024
const MB = KB * 1024
const GB = MB * 1024
const BASE_TIME = 1762500000 // 2025-11-07 前后，固定值保证可重复
const ROOT_PATH = '/home/zys'

function mulberry32(seed: number): () => number {
  let s = seed
  return () => {
    s = (s + 0x6d2b79f5) | 0
    let t = Math.imul(s ^ (s >>> 15), 1 | s)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

interface MNode {
  id: number
  name: string
  parent: number
  kind: NodeKind
  size: number
  ownSize: number
  files: number
  dirs: number
  mtime: number
  children: number[]
  errorCount: number
}

function basename(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean)
  return parts.length ? parts[parts.length - 1] : path
}

class TreeBuilder {
  nodes: MNode[] = []
  rng: () => number
  now: number

  constructor(seed: number) {
    this.rng = mulberry32(seed)
    this.now = BASE_TIME
  }

  add(name: string, kind: NodeKind, parent: number, size: number): number {
    const id = this.nodes.length
    this.nodes.push({
      id,
      name,
      parent,
      kind,
      size,
      ownSize: kind === 'file' ? size : 0,
      files: 0,
      dirs: 0,
      mtime: this.now - Math.floor(this.rng() * 60 * 86400),
      children: [],
      errorCount: 0,
    })
    if (parent >= 0) this.nodes[parent].children.push(id)
    return id
  }

  dir(parent: number, name: string, fn?: (id: number) => void): number {
    const id = this.add(name, 'dir', parent, 0)
    fn?.(id)
    return id
  }

  file(parent: number, name: string, size?: number): number {
    return this.add(name, 'file', parent, size ?? this.powerSize())
  }

  symlink(parent: number, name: string): number {
    return this.add(name, 'symlink', parent, 0)
  }

  /** 幂律：多数是 KB 级小文件，长尾到 GB。 */
  powerSize(): number {
    const r = this.rng()
    const bytes = 700 * Math.pow(1 - r, -1.35)
    return Math.max(1, Math.min(Math.floor(bytes), 2 * GB))
  }

  files(parent: number, count: number, prefix: string, ext: string, maxSize?: number): void {
    for (let i = 0; i < count; i++) {
      const size = maxSize ? Math.floor(this.rng() * maxSize) + 1 : undefined
      this.file(parent, `${prefix}-${i}.${ext}`, size)
    }
  }
}

function buildTemplate(targetNodes: number): MNode[] {
  const b = new TreeBuilder(0x5eed1234)
  const root = b.add('zys', 'dir', -1, 0)

  b.dir(root, 'Projects', (projects) => {
    b.dir(projects, 'disklens', (d) => {
      b.file(d, 'package.json', 4 * KB)
      b.file(d, 'README.md', 12 * KB)
      b.dir(d, 'app', (app) => {
        b.file(app, 'package-lock.json', 640 * KB)
        b.file(app, 'vite.config.ts', 3 * KB)
        b.dir(app, 'src', (src) => {
          b.files(src, 40, 'component', 'tsx', 40 * KB)
          b.files(src, 60, 'module', 'ts', 25 * KB)
          b.dir(src, 'assets', (assets) => b.files(assets, 80, 'icon', 'svg', 8 * KB))
        })
        b.dir(app, 'node_modules', (nm) => {
          for (let p = 0; p < 300; p++) {
            b.dir(nm, `pkg-${p}`, (pkg) => {
              b.files(pkg, 15, 'index', 'js', 90 * KB)
              b.dir(pkg, 'lib', (lib) => b.files(lib, 10, 'chunk', 'js', 200 * KB))
              b.dir(pkg, 'dist', (dist) => {
                b.files(dist, 6, 'bundle', 'mjs', 500 * KB)
                b.files(dist, 4, 'map', 'map', 300 * KB)
              })
              b.dir(pkg, 'types', (types) => b.files(types, 5, 'index', 'd.ts', 20 * KB))
            })
          }
        })
        b.dir(app, 'dist', (dist) => b.files(dist, 40, 'assets', 'js', 900 * KB))
      })
      b.dir(d, 'crates', (crates) => {
        b.dir(crates, 'disklens-core', (core) => {
          b.dir(core, 'src', (src) => b.files(src, 22, 'mod', 'rs', 60 * KB))
          b.files(core, 12, 'test', 'rs', 25 * KB)
        })
      })
      b.dir(d, 'target', (target) => {
        b.dir(target, 'debug', (debug) => {
          b.file(debug, 'disklens', 184 * MB)
          b.files(debug, 60, 'incremental', 'o', 40 * MB)
          b.dir(debug, 'deps', (deps) => b.files(deps, 120, 'libdisklens', 'rlib', 30 * MB))
        })
      })
      b.dir(d, 'docs', (docs) => b.files(docs, 8, 'design', 'md', 40 * KB))
    })

    b.dir(projects, 'reliability-tool', (rt) => {
      b.dir(rt, 'src', (src) => b.files(src, 80, 'reliability', 'py', 80 * KB))
      b.dir(rt, '.git', (git) => {
        b.files(git, 120, 'pack', 'pack', 18 * MB)
        b.files(git, 300, 'ref', 'txt', 6 * KB)
      })
      b.dir(rt, 'venv', (venv) => {
        b.dir(venv, 'lib', (lib) => {
          b.dir(lib, 'python3.11', (py) => {
            b.dir(py, 'site-packages', (sp) => {
              b.files(sp, 900, 'package', 'py', 120 * KB)
              b.files(sp, 120, 'native', 'so', 8 * MB)
            })
          })
        })
      })
      b.files(rt, 20, 'notebook', 'ipynb', 4 * MB)
    })

    for (let s = 0; s < 10; s++) {
      b.dir(projects, `side-project-${s}`, (sp) => {
        b.files(sp, 25, 'source', ['ts', 'py', 'rs'][s % 3], 60 * KB)
        b.dir(sp, '.git', (git) => b.files(git, 40, 'object', 'pack', 10 * MB))
      })
    }
  })

  b.dir(root, '.cache', (cache) => {
    b.dir(cache, 'pip', (pip) => {
      b.dir(pip, 'http-v2', (http) => b.files(http, 700, 'wheel', 'body', 3 * MB))
    })
    b.dir(cache, 'mozilla', (mz) => {
      b.dir(mz, 'firefox', (ff) => {
        b.dir(ff, 'cache2', (c2) => b.files(c2, 900, 'entry', 'bin', 2 * MB))
      })
    })
    b.dir(cache, 'thumbnails', (thumbs) => b.files(thumbs, 600, 'thumb', 'webp', 300 * KB))
    b.dir(cache, 'electron', (el) => b.files(el, 400, 'blob', 'cache', 6 * MB))
    b.dir(cache, 'locked', (locked) => {
      b.file(locked, 'locked.db', 42 * MB)
      b.files(locked, 8, 'held', 'lock', 4 * KB)
    })
    b.file(cache, 'tracker.db', 18 * MB)
  })

  b.dir(root, '.local', (local) => {
    b.dir(local, 'share', (share) => {
      b.dir(share, 'Steam', (steam) => {
        b.dir(steam, 'steamapps', (apps) => {
          b.dir(apps, 'common', (common) => {
            const games = [
              { name: 'Cyberpunk 2077', files: 45, size: 700 * MB },
              { name: 'Elden Ring', files: 38, size: 500 * MB },
              { name: 'Factorio', files: 22, size: 220 * MB },
              { name: 'Hades II', files: 18, size: 180 * MB },
            ]
            for (const g of games) {
              b.dir(common, g.name, (game) => {
                for (let i = 0; i < g.files; i++) b.file(game, `data-${i}.pak`, Math.floor(g.size * (0.3 + b.rng())))
                b.dir(game, 'bin', (bin) => b.files(bin, 8, 'game', 'so', 12 * MB))
              })
            }
          })
        })
      })
      b.dir(share, 'icons', (icons) => b.files(icons, 300, 'app', 'png', 40 * KB))
    })
  })

  b.dir(root, 'Downloads', (dl) => {
    b.file(dl, 'large.bin', 6 * GB + 720 * MB)
    b.file(dl, 'ubuntu-24.04.1-desktop-amd64.iso', 3 * GB + 100 * MB)
    b.file(dl, 'dataset-2025-08.tar.zst', 2 * GB + 400 * MB)
    b.file(dl, 'archive.zip', 780 * MB)
    b.file(dl, 'node-v22.23.1-linux-x64.tar.xz', 28 * MB)
    b.files(dl, 40, 'screenshot', 'png', 3 * MB)
  })

  b.dir(root, 'Videos', (v) => {
    for (let i = 0; i < 6; i++) b.file(v, `capture-2025-${String(i + 1).padStart(2, '0')}.mp4`, 600 * MB + Math.floor(b.rng() * 1200 * MB))
    b.dir(v, 'clips', (c) => b.files(c, 30, 'clip', 'mkv', 120 * MB))
  })

  b.dir(root, 'Pictures', (p) => {
    b.files(p, 220, 'IMG_2025', 'jpg', 6 * MB)
    b.files(p, 40, 'raw', 'dng', 28 * MB)
  })

  b.dir(root, 'Documents', (d) => {
    b.files(d, 80, 'report', 'pdf', 4 * MB)
    b.files(d, 40, 'note', 'md', 30 * KB)
    b.dir(d, 'tax', (tax) => b.files(tax, 20, 'receipt', 'pdf', 1 * MB))
  })

  b.dir(root, 'Music', (m) => b.files(m, 60, 'track', 'flac', 45 * MB))

  b.dir(root, '.config', (config) => {
    for (let i = 0; i < 8; i++) {
      b.dir(config, `app-${i}`, (app) => b.files(app, 12, 'config', 'json', 20 * KB))
    }
  })

  b.dir(root, '.ssh', (ssh) => {
    b.file(ssh, 'id_ed25519', 464)
    b.file(ssh, 'known_hosts', 12 * KB)
  })
  b.dir(root, '.hermes', (hermes) => b.files(hermes, 120, 'session', 'log', 800 * KB))
  b.file(root, 'notes.md', 24 * KB)
  b.symlink(root, 'latest-backup')

  // 补足到 ~20000 节点，缩略图形态的小文件，保持确定性。
  const thumbnails = b.nodes.find((n) => n.name === 'thumbnails')!.id
  let i = 0
  while (b.nodes.length < targetNodes) {
    const size = 2 * KB + Math.floor(b.rng() * 38 * KB)
    b.add(`thumb-${i++}.webp`, 'file', thumbnails, size)
  }

  // 逆序聚合（子节点索引恒大于父节点）。
  for (let id = b.nodes.length - 1; id > 0; id--) {
    const n = b.nodes[id]
    const p = b.nodes[n.parent]
    if (!p) continue
    p.size += n.size
    p.files += n.files + (n.kind === 'file' ? 1 : 0)
    p.dirs += n.dirs + (n.kind === 'dir' ? 1 : 0)
    p.errorCount += n.errorCount
  }
  return b.nodes
}

interface MockScan {
  scanId: string
  rootPath: string
  startedAt: number
  timer: ReturnType<typeof setInterval> | null
  cancelled: boolean
  progress: ScanProgress
  lastDone: ScanSummary | null
}

const PROTECTED = new Set(['/', '/usr', '/etc', '/bin', '/lib', '/var/lib', '/boot', '/System'])
const HOME_PROTECTED = new Set([ROOT_PATH, `${ROOT_PATH}/Projects`, `${ROOT_PATH}/.ssh`, `${ROOT_PATH}/.hermes`])

function guardReason(path: string, rootPath: string): string | null {
  if (!path || path.includes('..')) return '路径非法'
  if (PROTECTED.has(path)) return '系统路径受保护'
  if (HOME_PROTECTED.has(path) || path === rootPath) return '受保护目录，禁止删除'
  return null
}

export class MockBackend implements Backend {
  private nodes: MNode[]
  private scans = new Map<string, MockScan>()
  private progressSubs = new Set<(p: ScanProgress) => void>()
  private doneSubs = new Set<(s: ScanSummary) => void>()
  private errorSubs = new Set<(e: { scanId: string; message: string }) => void>()
  private deleted = new Set<string>()
  private snapshots: SnapshotInfo[] = []
  private seq = 0
  private scanMs: number
  private defaultPath: string

  constructor(opts: { seed?: number; targetNodes?: number; scanMs?: number; defaultPath?: string } = {}) {
    this.nodes = buildTemplate(opts.targetNodes ?? 20000)
    this.scanMs = opts.scanMs ?? 2500
    this.defaultPath = opts.defaultPath ?? ROOT_PATH
  }

  listDrives(): Promise<Drive[]> {
    return Promise.resolve([
      { name: 'nvme0n1p2', mountPoint: '/', totalBytes: 127 * GB, availableBytes: 11 * GB },
      { name: 'nvme0n1p3', mountPoint: '/home', totalBytes: 931 * GB, availableBytes: 204 * GB },
      { name: 'sda1', mountPoint: '/run/media/zys/backup', totalBytes: 1 * 1024 * GB, availableBytes: 350 * GB },
    ])
  }

  startScan(path: string, _options?: ScanOptions): Promise<{ scanId: string }> {
    const rootPath = path.trim() || this.defaultPath
    if (!rootPath.startsWith('/')) return Promise.reject(new Error('仅支持绝对路径'))
    const scanId = `mock-${++this.seq}`
    const startedAt = Date.now()
    const total = this.nodes[0].size
    const scan: MockScan = {
      scanId,
      rootPath,
      startedAt,
      timer: null,
      cancelled: false,
      progress: { scanId, entries: 0, bytes: 0, currentPath: rootPath, elapsedMs: 0 },
      lastDone: null,
    }
    this.scans.set(scanId, scan)
    const steps = Math.max(4, Math.round(this.scanMs / 200))
    const tickMs = Math.max(10, Math.round(this.scanMs / steps))
    let tick = 0
    const samplePaths = [
      `${rootPath}/Projects/disklens/app/node_modules`,
      `${rootPath}/.cache/pip/http-v2`,
      `${rootPath}/.local/share/Steam/steamapps/common`,
      `${rootPath}/Downloads`,
      `${rootPath}/Projects/disklens/target/debug`,
      `${rootPath}/Videos`,
      `${rootPath}/Pictures`,
    ]
    scan.timer = setInterval(() => {
      tick++
      const elapsed = Date.now() - startedAt
      const f = Math.min(0.97, tick / steps)
      scan.progress = {
        scanId,
        entries: Math.floor(this.nodes.length * f),
        bytes: Math.floor(total * f),
        currentPath: samplePaths[tick % samplePaths.length],
        elapsedMs: elapsed,
      }
      this.progressSubs.forEach((cb) => cb(scan.progress))
      if (tick >= steps && !scan.cancelled) this.finish(scan)
    }, tickMs)
    return Promise.resolve({ scanId })
  }

  cancelScan(scanId: string): Promise<void> {
    const scan = this.scans.get(scanId)
    if (!scan || !scan.timer) return Promise.resolve()
    scan.cancelled = true
    clearInterval(scan.timer)
    scan.timer = null
    setTimeout(() => this.finish(scan), 80)
    return Promise.resolve()
  }

  private finish(scan: MockScan): void {
    if (scan.timer) clearInterval(scan.timer)
    scan.timer = null
    if (scan.lastDone) return
    // shortcut: 取消后仍返回完整模板树（真实 core 返回已扫部分）；UI 靠 cancelled 标记区分。
    const root = this.rootView(scan)
    const summary: ScanSummary = {
      scanId: scan.scanId,
      rootPath: scan.rootPath,
      root,
      elapsedMs: Date.now() - scan.startedAt,
      entries: scan.cancelled ? scan.progress.entries : this.nodes.length,
      errors: scan.rootPath === '/' ? 12 : 3,
      cancelled: scan.cancelled,
    }
    scan.lastDone = summary
    this.doneSubs.forEach((cb) => cb(summary))
  }

  private getScan(scanId: string): MockScan {
    const scan = this.scans.get(scanId)
    if (!scan) throw new Error('scan not found')
    return scan
  }

  private view(n: MNode): NodeView {
    return {
      id: n.id,
      name: n.name,
      size: n.size,
      ownSize: n.ownSize,
      files: n.files,
      dirs: n.dirs,
      mtime: n.mtime,
      kind: n.kind,
      hasChildren: n.kind === 'dir' && n.children.length > 0,
      errorCount: n.errorCount,
    }
  }

  private rootView(scan: MockScan): NodeView {
    return { ...this.view(this.nodes[0]), name: basename(scan.rootPath) || scan.rootPath }
  }

  listChildren(args: ListChildrenArgs): Promise<{ total: number; items: NodeView[] }> {
    this.getScan(args.scanId)
    const parent = this.nodes[args.nodeId]
    if (!parent) throw new Error('node not found')
    const filter = (args.filter ?? '').toLowerCase()
    const minSize = args.minSize ?? 0
    let items = parent.children
      .map((id) => this.nodes[id])
      .filter((n) => (!filter || n.name.toLowerCase().includes(filter)) && n.size >= minSize)
    const dir = args.desc ? -1 : 1
    const sortBy: SortBy = args.sortBy ?? 'size'
    items = [...items].sort((a, b) => {
      let d = 0
      if (sortBy === 'name') d = a.name.localeCompare(b.name)
      else if (sortBy === 'files') d = a.files - b.files
      else if (sortBy === 'mtime') d = a.mtime - b.mtime
      else d = a.size - b.size
      if (d === 0) return a.id - b.id // 稳定排序
      return d * dir
    })
    const total = items.length
    const offset = Math.max(0, args.offset ?? 0)
    const limit = Math.max(0, Math.min(args.limit ?? 5000, 5000))
    return Promise.resolve({
      total,
      items: items.slice(offset, offset + limit).map((n) => this.view(n)),
    })
  }

  getNode(scanId: string, nodeId: number): Promise<NodeView> {
    this.getScan(scanId)
    const node = this.nodes[nodeId]
    if (!node) return Promise.reject(new Error('node not found'))
    return Promise.resolve(this.view(node))
  }

  nodePath(scanId: string, nodeId: number): Promise<string> {
    const scan = this.getScan(scanId)
    if (!this.nodes[nodeId]) return Promise.reject(new Error('node not found'))
    return Promise.resolve(this.pathOf(scan, nodeId))
  }

  private pathOf(scan: MockScan, nodeId: number): string {
    if (nodeId === 0) return scan.rootPath
    const node = this.nodes[nodeId]
    const parts: string[] = [node.name]
    let cur = node
    while (cur.parent > 0) {
      cur = this.nodes[cur.parent]
      parts.push(cur.name)
    }
    parts.reverse()
    return `${scan.rootPath.replace(/\/$/, '')}/${parts.join('/')}`
  }

  topFiles(scanId: string, limit: number, minSize = 0): Promise<TopFile[]> {
    const scan = this.getScan(scanId)
    const files = this.nodes.filter((n) => n.kind === 'file' && n.size >= minSize)
    files.sort((a, b) => (b.size - a.size) || (a.id - b.id))
    return Promise.resolve(files.slice(0, limit).map((n) => ({
      id: n.id,
      name: n.name,
      path: this.pathOf(scan, n.id),
      size: n.size,
      mtime: n.mtime,
    })))
  }

  typeStats(scanId: string, topN: number): Promise<TypeStat[]> {
    this.getScan(scanId)
    const map = new Map<string, TypeStat>()
    for (const n of this.nodes) {
      if (n.kind !== 'file') continue
      const dot = n.name.lastIndexOf('.')
      const ext = dot > 0 ? n.name.slice(dot + 1).toLowerCase() : ''
      const cur = map.get(ext) ?? { ext, bytes: 0, files: 0 }
      cur.bytes += n.size
      cur.files += 1
      map.set(ext, cur)
    }
    return Promise.resolve(
      [...map.values()].sort((a, b) => (b.bytes - a.bytes) || a.ext.localeCompare(b.ext)).slice(0, topN),
    )
  }

  saveSnapshot(scanId: string): Promise<{ path: string }> {
    const scan = this.getScan(scanId)
    // shortcut: 快照是内存记录，不落盘；快照 UI 本版未做，等快照页面时接真实后端。
    const path = `~/.local/share/disklens/snapshots/mock-${scanId}.json.gz`
    this.snapshots.push({
      path,
      rootPath: scan.rootPath,
      createdAt: Math.floor(Date.now() / 1000),
      totalBytes: this.nodes[0].size,
      entries: this.nodes.length,
    })
    return Promise.resolve({ path })
  }

  listSnapshots(): Promise<SnapshotInfo[]> {
    return Promise.resolve([...this.snapshots])
  }

  diffSnapshots(_aPath: string, _bPath: string, _limit: number): Promise<DiffResult> {
    // shortcut: Mock 只满足契约形状，不生成真实快照差异；快照对比 UI 本版未做。
    return Promise.resolve({ grown: [], shrunk: [], grownBytes: 0, shrunkBytes: 0 })
  }

  openPath(_path: string, _reveal = false): Promise<void> {
    // 浏览器没有文件管理器，Mock 只记录一次，方便调试。
    if (import.meta.env.DEV) console.info('[mock] openPath', _path, _reveal)
    return Promise.resolve()
  }

  private runDelete(paths: string[], confirmText: string | null, rootPath: string): OpResult {
    const result: OpResult = { ok: [], failed: [] }
    for (const path of paths) {
      const reason = guardReason(path, rootPath)
      if (reason) {
        result.failed.push({ path, error: reason })
      } else if (confirmText !== null && confirmText !== basename(path)) {
        result.failed.push({ path, error: '确认文本与目录名不一致' })
      } else if (path.includes('/locked') || basename(path).startsWith('locked')) {
        result.failed.push({ path, error: '文件被占用（模拟）' })
      } else if (this.deleted.has(path)) {
        result.failed.push({ path, error: '文件不存在' })
      } else {
        this.deleted.add(path)
        result.ok.push(path)
      }
    }
    return result
  }

  moveToTrash(paths: string[]): Promise<OpResult> {
    const scans = [...this.scans.values()]
    const rootPath = scans[scans.length - 1]?.rootPath ?? ROOT_PATH
    return Promise.resolve(this.runDelete(paths, null, rootPath))
  }

  deletePermanent(paths: string[], confirmText: string): Promise<OpResult> {
    const scans = [...this.scans.values()]
    const rootPath = scans[scans.length - 1]?.rootPath ?? ROOT_PATH
    return Promise.resolve(this.runDelete(paths, confirmText, rootPath))
  }

  platformInfo(): Promise<PlatformInfo> {
    return Promise.resolve({ os: 'linux', homeDir: ROOT_PATH })
  }

  onProgress(cb: (p: ScanProgress) => void): () => void {
    this.progressSubs.add(cb)
    return () => this.progressSubs.delete(cb)
  }

  onDone(cb: (s: ScanSummary) => void): () => void {
    this.doneSubs.add(cb)
    return () => this.doneSubs.delete(cb)
  }

  onError(cb: (e: { scanId: string; message: string }) => void): () => void {
    this.errorSubs.add(cb)
    return () => this.errorSubs.delete(cb)
  }
}

export const mockBackend = new MockBackend()
