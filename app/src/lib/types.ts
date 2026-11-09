// IPC 契约镜像 —— 与 docs/03-架构设计.md §4 逐字一致。
export type NodeKind = 'dir' | 'file' | 'symlink'

export interface NodeView {
  id: number
  name: string
  size: number
  ownSize: number
  files: number
  dirs: number
  mtime: number
  kind: NodeKind
  hasChildren: boolean
  errorCount: number
}

export interface Drive {
  name: string
  mountPoint: string
  totalBytes: number
  availableBytes: number
}

export interface ScanSummary {
  scanId: string
  rootPath: string
  root: NodeView
  elapsedMs: number
  entries: number
  errors: number
  cancelled: boolean
}

export type SortBy = 'size' | 'name' | 'files' | 'mtime'

export interface ScanOptions {
  crossFilesystems?: boolean
  followSymlinks?: boolean
  exclude?: string[]
}

export interface ScanProgress {
  scanId: string
  entries: number
  bytes: number
  currentPath: string
  elapsedMs: number
}

export interface TopFile {
  id: number
  name: string
  path: string
  size: number
  mtime: number
}

export interface TypeStat {
  ext: string
  bytes: number
  files: number
}

export interface OpResult {
  ok: string[]
  failed: { path: string; error: string }[]
}

export interface SnapshotInfo {
  path: string
  rootPath: string
  createdAt: number
  totalBytes: number
  entries: number
}

export interface DiffRow {
  path: string
  sizeA: number
  sizeB: number
  delta: number
}

export interface DiffResult {
  grown: DiffRow[]
  shrunk: DiffRow[]
  grownBytes: number
  shrunkBytes: number
}

export interface ListChildrenArgs {
  scanId: string
  nodeId: number
  sortBy: SortBy
  desc: boolean
  offset: number
  limit: number
  minSize?: number
  filter?: string
}

export interface PlatformInfo {
  os: 'linux' | 'macos' | 'windows'
  homeDir: string
}

type Unlisten = () => void

export interface Backend {
  listDrives(): Promise<Drive[]>
  startScan(path: string, options?: ScanOptions): Promise<{ scanId: string }>
  cancelScan(scanId: string): Promise<void>
  listChildren(args: ListChildrenArgs): Promise<{ total: number; items: NodeView[] }>
  getNode(scanId: string, nodeId: number): Promise<NodeView>
  nodePath(scanId: string, nodeId: number): Promise<string>
  topFiles(scanId: string, limit: number, minSize?: number): Promise<TopFile[]>
  typeStats(scanId: string, topN: number): Promise<TypeStat[]>
  saveSnapshot(scanId: string): Promise<{ path: string }>
  listSnapshots(): Promise<SnapshotInfo[]>
  diffSnapshots(aPath: string, bPath: string, limit: number): Promise<DiffResult>
  openPath(path: string, reveal?: boolean): Promise<void>
  moveToTrash(paths: string[]): Promise<OpResult>
  deletePermanent(paths: string[], confirmText: string): Promise<OpResult>
  platformInfo(): Promise<PlatformInfo>
  onProgress(cb: (p: ScanProgress) => void): Unlisten
  onDone(cb: (s: ScanSummary) => void): Unlisten
  onError(cb: (e: { scanId: string; message: string }) => void): Unlisten
}
