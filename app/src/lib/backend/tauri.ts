import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  Backend,
  DiffResult,
  Drive,
  ListChildrenArgs,
  NodeView,
  OpResult,
  PlatformInfo,
  ScanOptions,
  ScanProgress,
  ScanSummary,
  SnapshotInfo,
  TopFile,
  TypeStat,
} from '../types'

function subscribe<T>(event: string, cb: (payload: T) => void): () => void {
  let off: UnlistenFn | null = null
  let disposed = false
  void listen<T>(event, (e) => cb(e.payload)).then((fn) => {
    if (disposed) fn()
    else off = fn
  })
  return () => {
    disposed = true
    off?.()
  }
}

export const tauriBackend: Backend = {
  listDrives: () => invoke<Drive[]>('list_drives'),
  startScan: (path: string, options?: ScanOptions) =>
    invoke<{ scanId: string }>('start_scan', { path, options }),
  cancelScan: (scanId: string) => invoke<void>('cancel_scan', { scanId }),
  listChildren: (args: ListChildrenArgs) =>
    invoke<{ total: number; items: NodeView[] }>('list_children', { ...args }),
  getNode: (scanId: string, nodeId: number) => invoke<NodeView>('get_node', { scanId, nodeId }),
  nodePath: (scanId: string, nodeId: number) => invoke<string>('node_path', { scanId, nodeId }),
  topFiles: (scanId: string, limit: number, minSize?: number) =>
    invoke<TopFile[]>('top_files', { scanId, limit, minSize }),
  typeStats: (scanId: string, topN: number) => invoke<TypeStat[]>('type_stats', { scanId, topN }),
  saveSnapshot: (scanId: string) => invoke<{ path: string }>('save_snapshot', { scanId }),
  listSnapshots: () => invoke<SnapshotInfo[]>('list_snapshots'),
  diffSnapshots: (aPath: string, bPath: string, limit: number) =>
    invoke<DiffResult>('diff_snapshots', { aPath, bPath, limit }),
  openPath: (path: string, reveal = false) => invoke<void>('open_path', { path, reveal }),
  moveToTrash: (paths: string[]) => invoke<OpResult>('move_to_trash', { paths }),
  deletePermanent: (paths: string[], confirmText: string) =>
    invoke<OpResult>('delete_permanent', { paths, confirmText }),
  platformInfo: () => invoke<PlatformInfo>('platform_info'),
  pickFolder: () => invoke<string | null>('pick_folder'),
  onProgress: (cb: (p: ScanProgress) => void) => subscribe('scan://progress', cb),
  onDone: (cb: (s: ScanSummary) => void) => subscribe('scan://done', cb),
  onError: (cb: (e: { scanId: string; message: string }) => void) => subscribe('scan://error', cb),
}
