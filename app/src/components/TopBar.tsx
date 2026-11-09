import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useScanStore } from '../store/scanStore'
import { backend, isTauri } from '../lib/backend'
import { formatBytes } from '../lib/format'
import { Button } from './ui'
import { SearchIcon } from './icons'
import { ScanProgress } from './ScanProgress'

const MB = 1024 * 1024

export function TopBar() {
  const drives = useScanStore((s) => s.drives)
  const rootPath = useScanStore((s) => s.rootPath)
  const status = useScanStore((s) => s.status)
  const filter = useScanStore((s) => s.filter)
  const minSize = useScanStore((s) => s.minSize)
  const startScan = useScanStore((s) => s.startScan)
  const setFilter = useScanStore((s) => s.setFilter)
  const setMinSize = useScanStore((s) => s.setMinSize)

  const [path, setPath] = useState(rootPath)
  // shortcut: 契约的 filter 只匹配当前目录直属子项，搜索不递归；后端提供 search 命令后升级。
  const [q, setQ] = useState(filter)

  useEffect(() => setPath(rootPath), [rootPath])
  useEffect(() => setQ(filter), [filter])
  useEffect(() => {
    const id = setTimeout(() => void setFilter(q), 100)
    return () => clearTimeout(id)
  }, [q, setFilter])

  const selectedDrive = drives
    .filter((d) => path.startsWith(d.mountPoint))
    .sort((a, b) => b.mountPoint.length - a.mountPoint.length)[0]

  const scan = (target: string) => {
    setPath(target)
    if (status !== 'scanning') void startScan(target)
  }

  const pickFolder = async () => {
    const picked = await backend.pickFolder().catch(() => null)
    if (picked) scan(picked)
  }

  // 窗口拖入文件夹直接开扫；over/leave 事件忽略
  useEffect(() => {
    if (!isTauri()) return
    let off: (() => void) | undefined
    let disposed = false
    void getCurrentWindow()
      .onDragDropEvent((e) => {
        const dropped = e.payload.type === 'drop' ? e.payload.paths[0] : undefined
        const s = useScanStore.getState()
        if (dropped && s.status !== 'scanning') void s.startScan(dropped)
      })
      .then((fn) => (disposed ? fn() : (off = fn)))
    return () => {
      disposed = true
      off?.()
    }
  }, [])

  return (
    <>
      <header className="flex h-11 shrink-0 items-center gap-2 border-b border-line bg-panel px-3">
        <div className="mr-1 flex select-none items-center gap-1.5">
          <span className="grid h-5 w-5 place-items-center rounded-md bg-accent text-[11px] font-bold text-white shadow-sm">
            ◧
          </span>
          <span className="text-[13px] font-semibold tracking-tight">DiskLens</span>
        </div>

        <select
          aria-label="选择磁盘"
          className="h-7 w-[168px] shrink-0 truncate rounded-md border border-line bg-panel2 px-1.5 text-xs text-fg outline-none focus:border-accent"
          value={selectedDrive?.mountPoint ?? ''}
          onChange={(e) => setPath(e.target.value || '/')}
        >
          {!selectedDrive && <option value="">磁盘…</option>}
          {drives.map((d) => (
            <option key={d.mountPoint} value={d.mountPoint}>
              {d.name} · {d.mountPoint} · {formatBytes(d.totalBytes - d.availableBytes)}/{formatBytes(d.totalBytes)}
            </option>
          ))}
        </select>

        <input
          aria-label="扫描目录"
          spellCheck={false}
          value={path}
          onChange={(e) => setPath(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && status !== 'scanning') void startScan(path)
          }}
          className="h-7 min-w-0 flex-1 rounded-md border border-line bg-panel2 px-2 font-mono text-xs text-fg outline-none placeholder:text-muted focus:border-accent"
          placeholder="/home/zys"
        />

        <Button variant="default" onClick={() => void pickFolder()} title="选择要扫描的目录">
          选目录
        </Button>

        <Button variant="primary" disabled={status === 'scanning'} onClick={() => void startScan(path)}>
          {status === 'scanning' ? '扫描中…' : '扫描'}
        </Button>

        <div className="relative ml-1 w-[190px] shrink-0">
          <SearchIcon size={13} className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted" />
          <input
            aria-label="搜索"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            disabled={status !== 'done'}
            placeholder="搜索当前树…"
            spellCheck={false}
            className="h-7 w-full rounded-md border border-line bg-panel2 pl-7 pr-2 text-xs text-fg outline-none placeholder:text-muted focus:border-accent disabled:opacity-50"
          />
        </div>

        <select
          aria-label="大小过滤"
          value={minSize}
          onChange={(e) => void setMinSize(Number(e.target.value))}
          disabled={status !== 'done'}
          className="h-7 shrink-0 rounded-md border border-line bg-panel2 px-1.5 text-xs text-muted outline-none focus:border-accent disabled:opacity-50"
        >
          <option value={0}>全部大小</option>
          <option value={MB}>≥1 MB</option>
          <option value={10 * MB}>≥10 MB</option>
          <option value={100 * MB}>≥100 MB</option>
        </select>
      </header>
      {status === 'scanning' && <ScanProgress />}
    </>
  )
}
