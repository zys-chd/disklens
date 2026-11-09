import { create } from 'zustand'
import { backend } from '../lib/backend'
import { childKey, type Row } from '../lib/tree'
import { confirmValid, type DeleteItem, type DeleteMode } from '../lib/delete'
import type {
  Drive,
  NodeView,
  OpResult,
  ScanProgress,
  ScanSummary,
  SortBy,
  TopFile,
  TypeStat,
} from '../lib/types'

export interface SelItem {
  id: number
  name: string
  size: number
}

export interface DialogState {
  mode: DeleteMode
  items: DeleteItem[]
  results: OpResult | null
  busy: boolean
}

export interface ToastState {
  id: number
  kind: 'info' | 'success' | 'error' | 'warn'
  text: string
}

type Status = 'idle' | 'scanning' | 'done' | 'error'

interface ScanState {
  drives: Drive[]
  scanId: string | null
  rootPath: string
  status: Status
  progress: ScanProgress | null
  summary: ScanSummary | null
  error: string
  expanded: Set<number>
  children: Record<string, NodeView[]>
  loading: Set<string>
  sortBy: SortBy
  desc: boolean
  filter: string
  minSize: number
  selection: Record<number, SelItem>
  lastSelectedId: number | null
  chain: NodeView[]
  scrollTarget: number | null
  topLimit: number
  topFiles: TopFile[]
  typeStats: TypeStat[]
  contextMenu: { x: number; y: number; node: NodeView } | null
  dialog: DialogState | null
  toast: ToastState | null

  init(): Promise<void>
  startScan(path: string): Promise<void>
  cancelScan(): Promise<void>
  keyFor(nodeId: number): string
  ensureChildren(nodeId: number): Promise<void>
  reloadChildren(): Promise<void>
  toggleExpand(nodeId: number): Promise<void>
  setSort(sortBy: SortBy): Promise<void>
  setFilter(filter: string): Promise<void>
  setMinSize(minSize: number): Promise<void>
  focusNode(node: NodeView, scroll?: boolean, chainOverride?: NodeView[]): Promise<void>
  selectNode(node: NodeView, mode: 'single' | 'toggle' | 'range', rows: Row[]): void
  clearSelection(): void
  reveal(path: string): Promise<void>
  requestDelete(mode: DeleteMode): Promise<void>
  runDelete(confirmText: string, only?: DeleteItem[]): Promise<void>
  retryFailed(confirmText: string): Promise<void>
  closeDialog(): void
  setTopLimit(limit: number): Promise<void>
  loadTop(): Promise<void>
  loadTypes(): Promise<void>
  openContextMenu(x: number, y: number, node: NodeView): void
  closeContextMenu(): void
  notify(kind: ToastState['kind'], text: string): void
}

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

let started = false

function chainToTarget(
  targetId: number,
  root: NodeView | undefined,
  children: Record<string, NodeView[]>,
  keyFor: (id: number) => string,
): NodeView[] | null {
  if (!root) return null
  if (targetId === root.id) return [root]
  const parent = new Map<number, NodeView>()
  const queue: NodeView[] = [root]
  const seen = new Set<number>([root.id])
  while (queue.length) {
    const cur = queue.shift()!
    const kids = children[keyFor(cur.id)] ?? []
    for (const kid of kids) {
      if (seen.has(kid.id)) continue
      seen.add(kid.id)
      parent.set(kid.id, cur)
      if (kid.id === targetId) {
        const chain = [kid]
        let p = cur
        while (p) {
          chain.unshift(p)
          const pp = parent.get(p.id)
          if (!pp) break
          p = pp
        }
        return chain
      }
      if (kid.hasChildren) queue.push(kid)
    }
  }
  return null
}

export const useScanStore = create<ScanState>()((set, get) => ({
  drives: [],
  scanId: null,
  rootPath: '',
  status: 'idle',
  progress: null,
  summary: null,
  error: '',
  expanded: new Set<number>(),
  children: {},
  loading: new Set<string>(),
  sortBy: 'size',
  desc: true,
  filter: '',
  minSize: 0,
  selection: {},
  lastSelectedId: null,
  chain: [],
  scrollTarget: null,
  topLimit: 50,
  topFiles: [],
  typeStats: [],
  contextMenu: null,
  dialog: null,
  toast: null,

  init: async () => {
    if (started) return
    started = true
    backend.onProgress((p) => {
      if (p.scanId !== get().scanId) return
      set({ progress: p })
    })
    backend.onDone(async (summary) => {
      if (summary.scanId !== get().scanId) return
      set({
        status: 'done',
        summary,
        progress: null,
        rootPath: summary.rootPath,
        chain: [summary.root],
        expanded: new Set<number>(),
        children: {},
        loading: new Set<string>(),
        selection: {},
        lastSelectedId: null,
        scrollTarget: null,
      })
      if (summary.cancelled) get().notify('warn', '扫描已取消，展示已扫到的部分')
      await get().ensureChildren(0)
      await Promise.all([get().loadTop(), get().loadTypes()])
    })
    backend.onError((e) => {
      if (e.scanId !== get().scanId) return
      set({ status: 'error', error: e.message })
    })
    try {
      const [drives, info] = await Promise.all([backend.listDrives(), backend.platformInfo()])
      set({ drives })
      await get().startScan(info.homeDir)
    } catch (e) {
      set({ status: 'error', error: errMsg(e) })
    }
  },

  startScan: async (path) => {
    const target = path.trim()
    if (!target) return
    const prev = get().scanId
    if (prev) void backend.cancelScan(prev).catch(() => {})
    set({
      scanId: null,
      rootPath: target,
      status: 'scanning',
      progress: null,
      summary: null,
      error: '',
      children: {},
      loading: new Set<string>(),
      expanded: new Set<number>(),
      selection: {},
      lastSelectedId: null,
      chain: [],
      scrollTarget: null,
      dialog: null,
      contextMenu: null,
      topFiles: [],
      typeStats: [],
    })
    try {
      const { scanId } = await backend.startScan(target)
      set({ scanId })
    } catch (e) {
      set({ status: 'error', error: errMsg(e) })
    }
  },

  cancelScan: async () => {
    const id = get().scanId
    if (!id) return
    try {
      await backend.cancelScan(id)
    } catch (e) {
      get().notify('error', errMsg(e))
    }
  },

  keyFor: (nodeId) => {
    const { sortBy, desc, filter, minSize } = get()
    return childKey(nodeId, sortBy, desc, filter, minSize)
  },

  ensureChildren: async (nodeId) => {
    const { scanId, sortBy, desc, filter, minSize } = get()
    if (!scanId) return
    const key = childKey(nodeId, sortBy, desc, filter, minSize)
    if (get().children[key] !== undefined || get().loading.has(key)) return
    set((s) => ({ loading: new Set(s.loading).add(key) }))
    try {
      const res = await backend.listChildren({
        scanId,
        nodeId,
        sortBy,
        desc,
        offset: 0,
        limit: 5000,
        filter: filter || undefined,
        minSize: minSize || undefined,
      })
      if (get().scanId !== scanId) return
      set((s) => {
        const loading = new Set(s.loading)
        loading.delete(key)
        return { children: { ...s.children, [key]: res.items }, loading }
      })
    } catch (e) {
      set((s) => {
        const loading = new Set(s.loading)
        loading.delete(key)
        return { loading }
      })
      get().notify('error', errMsg(e))
    }
  },

  reloadChildren: async () => {
    const expanded = [...get().expanded]
    set({ children: {}, loading: new Set<string>() })
    await get().ensureChildren(0)
    await Promise.all(expanded.map((id) => get().ensureChildren(id)))
  },

  toggleExpand: async (nodeId) => {
    const expanded = new Set(get().expanded)
    if (expanded.has(nodeId)) {
      expanded.delete(nodeId)
      set({ expanded })
      return
    }
    expanded.add(nodeId)
    set({ expanded })
    await get().ensureChildren(nodeId)
  },

  setSort: async (sortBy) => {
    const { sortBy: cur, desc } = get()
    const nextDesc = sortBy === cur ? !desc : sortBy !== 'name'
    set({ sortBy, desc: nextDesc })
    await get().reloadChildren()
  },

  setFilter: async (filter) => {
    if (filter === get().filter) return
    set({ filter, scrollTarget: null })
    await get().reloadChildren()
  },

  setMinSize: async (minSize) => {
    if (minSize === get().minSize) return
    set({ minSize })
    await get().reloadChildren()
  },

  focusNode: async (node, scroll = false, chainOverride) => {
    const root = get().summary?.root
    let chain: NodeView[] | null = chainOverride ?? null
    if (!chain) {
      chain = chainToTarget(node.id, root, get().children, get().keyFor)
      if (!chain) {
        const existing = get().chain.findIndex((n) => n.id === node.id)
        chain = existing >= 0 ? get().chain.slice(0, existing + 1) : [...get().chain, node]
      }
    }
    set({ chain })
    if (node.kind === 'dir' && node.hasChildren) {
      set((s) => ({ expanded: new Set(s.expanded).add(node.id) }))
      await get().ensureChildren(node.id)
    }
    if (scroll) set({ scrollTarget: node.id })
  },

  selectNode: (node, mode, rows) => {
    const item: SelItem = { id: node.id, name: node.name, size: node.size }
    if (mode === 'single') {
      set({ selection: { [node.id]: item }, lastSelectedId: node.id })
      return
    }
    if (mode === 'toggle') {
      const selection = { ...get().selection }
      if (selection[node.id]) delete selection[node.id]
      else selection[node.id] = item
      set({ selection, lastSelectedId: node.id })
      return
    }
    const from = rows.findIndex((r) => r.node.id === get().lastSelectedId)
    const to = rows.findIndex((r) => r.node.id === node.id)
    if (from < 0 || to < 0) {
      set({ selection: { [node.id]: item }, lastSelectedId: node.id })
      return
    }
    const [a, b] = from < to ? [from, to] : [to, from]
    const selection: Record<number, SelItem> = {}
    for (let i = a; i <= b; i++) {
      const n = rows[i].node
      selection[n.id] = { id: n.id, name: n.name, size: n.size }
    }
    set({ selection })
  },

  clearSelection: () => set({ selection: {}, lastSelectedId: null }),

  reveal: async (path) => {
    const { scanId, summary, rootPath } = get()
    if (!scanId || !summary) return
    if (get().filter) {
      set({ filter: '' })
      await get().reloadChildren()
    }
    const rel = path.startsWith(rootPath) ? path.slice(rootPath.length) : path
    const segments = rel.split('/').filter(Boolean)
    let cur = summary.root
    const chain = [cur]
    const expanded = new Set(get().expanded)
    for (const segment of segments) {
      if (cur.kind !== 'dir') break
      await get().ensureChildren(cur.id)
      const kids = get().children[get().keyFor(cur.id)] ?? []
      const child = kids.find((k) => k.name === segment)
      if (!child) break
      chain.push(child)
      cur = child
      if (child.kind === 'dir') {
        expanded.add(child.id)
        await get().ensureChildren(child.id)
      }
    }
    const isFile = cur.kind === 'file'
    const viewChain = isFile && chain.length > 1 ? chain.slice(0, -1) : chain
    set({
      chain: viewChain,
      expanded,
      scrollTarget: cur.id,
      ...(isFile
        ? { selection: { [cur.id]: { id: cur.id, name: cur.name, size: cur.size } }, lastSelectedId: cur.id }
        : {}),
    })
  },

  requestDelete: async (mode) => {
    const { scanId, selection } = get()
    if (!scanId) return
    const ids = Object.keys(selection).map(Number)
    if (!ids.length) return
    try {
      const paths = await Promise.all(ids.map((id) => backend.nodePath(scanId, id)))
      const items: DeleteItem[] = ids.map((id, i) => ({ ...selection[id], path: paths[i] }))
      set({ dialog: { mode, items, results: null, busy: false }, contextMenu: null })
    } catch (e) {
      get().notify('error', errMsg(e))
    }
  },

  runDelete: async (confirmText, only) => {
    const dialog = get().dialog
    const scanId = get().scanId
    if (!dialog || dialog.busy || !scanId) return
    const items = only ?? dialog.items
    if (!confirmValid(dialog.mode, items, confirmText)) return
    set({ dialog: { ...dialog, busy: true } })
    try {
      const paths = items.map((i) => i.path)
      const res =
        dialog.mode === 'trash'
          ? await backend.moveToTrash(paths)
          : await backend.deletePermanent(paths, confirmText)
      const okSet = new Set(res.ok)
      const prevOk = dialog.results?.ok ?? []
      const merged: OpResult = { ok: [...new Set([...prevOk, ...res.ok])], failed: res.failed }
      set((s) => {
        const selection = { ...s.selection }
        for (const item of items) if (okSet.has(item.path)) delete selection[item.id]
        return {
          dialog: s.dialog ? { ...s.dialog, results: merged, busy: false } : null,
          selection,
        }
      })
      const failed = res.failed.length
      get().notify(
        failed ? 'warn' : 'success',
        failed ? `${res.ok.length} 项成功，${failed} 项失败` : `${res.ok.length} 项已删除`,
      )
    } catch (e) {
      set((s) => ({ dialog: s.dialog ? { ...s.dialog, busy: false } : null }))
      get().notify('error', errMsg(e))
    }
  },

  retryFailed: async (confirmText) => {
    const dialog = get().dialog
    if (!dialog?.results) return
    const failed = new Set(dialog.results.failed.map((f) => f.path))
    const items = dialog.items.filter((i) => failed.has(i.path))
    await get().runDelete(confirmText, items)
  },

  closeDialog: () => set({ dialog: null }),

  setTopLimit: async (limit) => {
    set({ topLimit: limit })
    await get().loadTop()
  },

  loadTop: async () => {
    const { scanId, topLimit } = get()
    if (!scanId) return
    try {
      const rows = await backend.topFiles(scanId, topLimit)
      if (get().scanId === scanId) set({ topFiles: rows })
    } catch (e) {
      get().notify('error', errMsg(e))
    }
  },

  loadTypes: async () => {
    const { scanId } = get()
    if (!scanId) return
    try {
      const rows = await backend.typeStats(scanId, 12)
      if (get().scanId === scanId) set({ typeStats: rows })
    } catch (e) {
      get().notify('error', errMsg(e))
    }
  },

  openContextMenu: (x, y, node) => set({ contextMenu: { x, y, node } }),
  closeContextMenu: () => set({ contextMenu: null }),

  notify: (kind, text) => {
    const id = Date.now() + Math.random()
    set({ toast: { id, kind, text } })
    setTimeout(() => {
      if (get().toast?.id === id) set({ toast: null })
    }, 3200)
  },
}))
