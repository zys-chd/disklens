import { useCallback, useMemo, useRef, useEffect } from 'react'
import type { CSSProperties } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useScanStore } from '../store/scanStore'
import { flattenRows, type Row } from '../lib/tree'
import { formatBytes, formatCount, formatMtime } from '../lib/format'
import type { NodeView, SortBy } from '../lib/types'
import { ChevronRight, FileIcon, FolderIcon } from './icons'
import { Button, Empty, Spinner } from './ui'

const GRID = 'grid grid-cols-[minmax(0,1fr)_132px_64px_88px] items-center'

const SORTS: { key: SortBy; label: string; align: string }[] = [
  { key: 'name', label: '名称', align: 'text-left' },
  { key: 'size', label: '大小', align: 'text-right' },
  { key: 'files', label: '文件数', align: 'text-right' },
  { key: 'mtime', label: '时间', align: 'text-right' },
]

export function TreeView() {
  const rowsState = useScanStore()
  const {
    status,
    summary,
    expanded,
    children,
    loading,
    sortBy,
    desc,
    filter,
    selection,
    scrollTarget,
    keyFor,
    setSort,
    toggleExpand,
    focusNode,
    selectNode,
    openContextMenu,
    startScan,
    rootPath,
  } = rowsState

  const scrollRef = useRef<HTMLDivElement>(null)

  const rootChildren = children[keyFor(0)] ?? []
  const rows = useMemo(
    () => flattenRows(rootChildren, (id) => children[keyFor(id)], expanded),
    // keyFor 由下面这些字段派生
    [rootChildren, children, expanded, sortBy, desc, filter, rowsState.minSize, keyFor],
  )

  const parentSize = useMemo(() => {
    const map = new Map<number, number>()
    const stack: number[] = []
    const rootSize = summary?.root.size ?? 0
    for (const row of rows) {
      stack.length = row.depth
      const p = row.depth === 0 ? rootSize : (stack[row.depth - 1] ?? rootSize)
      map.set(row.node.id, p)
      stack[row.depth] = row.node.size
    }
    return map
  }, [rows, summary])

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 28,
    overscan: 8,
  })

  useEffect(() => {
    if (scrollTarget == null) return
    const index = rows.findIndex((r) => r.node.id === scrollTarget)
    if (index >= 0) virtualizer.scrollToIndex(index, { align: 'center' })
    const timer = setTimeout(() => useScanStore.setState({ scrollTarget: null }), 60)
    return () => clearTimeout(timer)
  }, [scrollTarget, rows, virtualizer])

  const onRowClick = useCallback(
    (node: NodeView, e: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }) => {
      const mode = e.shiftKey ? 'range' : e.ctrlKey || e.metaKey ? 'toggle' : 'single'
      selectNode(node, mode, rows)
    },
    [selectNode, rows],
  )

  const loadingRoot = loading.has(keyFor(0))

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className={`${GRID} h-7 shrink-0 border-b border-line bg-panel2 pr-2 text-[11px] text-muted`}>
        {SORTS.map((s) => (
          <button
            key={s.key}
            type="button"
            onClick={() => void setSort(s.key)}
            className={`${s.align} flex h-7 items-center px-2 font-medium hover:text-fg ${
              s.key === 'size' || s.key === 'files' || s.key === 'mtime' ? 'justify-end' : ''
            }`}
          >
            {s.label}
            <span className="ml-1 w-2 text-accent">{sortBy === s.key ? (desc ? '↓' : '↑') : ''}</span>
          </button>
        ))}
      </div>

      <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto" style={{ scrollbarGutter: 'stable' }}>
        {status === 'scanning' ? (
          <ScanningSkeleton />
        ) : status === 'error' ? (
          <div className="flex h-full flex-col items-center justify-center gap-2">
            <Empty title="扫描出错" hint={rowsState.error || '未知错误'} />
            <Button variant="primary" onClick={() => void startScan(rootPath)}>
              重试
            </Button>
          </div>
        ) : status === 'idle' ? (
          <div className="flex h-full items-center justify-center gap-2 text-xs text-muted">
            <Spinner />
            准备中…
          </div>
        ) : loadingRoot && rows.length === 0 ? (
          <div className="flex h-full items-center justify-center gap-2 text-xs text-muted">
            <Spinner />
            读取目录…
          </div>
        ) : rows.length === 0 ? (
          <Empty
            title={filter || rowsState.minSize ? '没有匹配的条目' : '此目录为空'}
            hint={filter ? `过滤条件：${filter}` : undefined}
          />
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {virtualizer.getVirtualItems().map((item) => {
              const row = rows[item.index]
              return (
                <TreeRow
                  key={row.node.id}
                  row={row}
                  parentSize={parentSize.get(row.node.id) ?? row.node.size}
                  expanded={expanded.has(row.node.id)}
                  selected={!!selection[row.node.id]}
                  style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: 28, transform: `translateY(${item.start}px)` }}
                  onClick={(e) => onRowClick(row.node, e)}
                  onDoubleClick={() => row.node.kind === 'dir' && void focusNode(row.node, true)}
                  onToggle={() => void toggleExpand(row.node.id)}
                  onContextMenu={(e) => {
                    e.preventDefault()
                    openContextMenu(e.clientX, e.clientY, row.node)
                  }}
                />
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}

function ScanningSkeleton() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-xs text-muted">
      <Spinner size={20} />
      <div>正在扫描并构建目录树…</div>
      <div className="flex flex-col gap-1.5 opacity-50">
        {[220, 160, 190, 120].map((w, i) => (
          <div key={i} className="h-2 rounded bg-line" style={{ width: w }} />
        ))}
      </div>
    </div>
  )
}

function TreeRow({
  row,
  parentSize,
  expanded,
  selected,
  style,
  onClick,
  onDoubleClick,
  onToggle,
  onContextMenu,
}: {
  row: Row
  parentSize: number
  expanded: boolean
  selected: boolean
  style: CSSProperties
  onClick: (e: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }) => void
  onDoubleClick: () => void
  onToggle: () => void
  onContextMenu: (e: React.MouseEvent) => void
}) {
  const { node, depth } = row
  const isDir = node.kind === 'dir'
  const ratio = parentSize > 0 ? Math.min(100, (node.size / parentSize) * 100) : 0

  return (
    <div
      role="row"
      aria-selected={selected}
      tabIndex={0}
      title={`${node.name}\n${formatBytes(node.size)} · ${formatCount(node.files)} 个文件`}
      style={style}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      onKeyDown={(e) => {
        if (e.key === 'Enter') onClick(e)
      }}
      className={`${GRID} h-7 cursor-default select-none text-[12px] outline-none transition-colors ${
        selected ? 'bg-sel' : 'hover:bg-hover'
      }`}
    >
      <div className="flex min-w-0 items-center" style={{ paddingLeft: 4 + depth * 14 }}>
        <button
          type="button"
          tabIndex={-1}
          aria-label={expanded ? '收起' : '展开'}
          onClick={(e) => {
            e.stopPropagation()
            onToggle()
          }}
          className={`mr-0.5 grid h-4 w-4 shrink-0 place-items-center rounded text-muted hover:text-fg ${
            isDir && node.hasChildren ? '' : 'invisible'
          }`}
        >
          <ChevronRight size={11} className={expanded ? 'rotate-90 transition-transform' : 'transition-transform'} />
        </button>
        <span className={`mr-1.5 shrink-0 ${isDir ? 'text-accent' : 'text-muted/70'}`}>
          {isDir ? <FolderIcon size={13} /> : <FileIcon size={13} />}
        </span>
        <span className={`truncate ${isDir ? 'font-medium' : 'text-fg/90'}`}>{node.name}</span>
        {node.kind === 'symlink' && <span className="ml-1 shrink-0 text-[10px] text-muted">link</span>}
      </div>

      <div className="relative flex h-full items-center justify-end px-2">
        <div
          className="absolute bottom-[3px] left-2 h-[2px] rounded-full bg-accent/35"
          style={{ width: `${ratio}%` }}
        />
        <span className="relative tabular-nums text-fg/90">{formatBytes(node.size)}</span>
      </div>

      <div className="px-2 text-right tabular-nums text-muted">{node.files ? formatCount(node.files) : '—'}</div>
      <div className="px-2 text-right text-[11px] tabular-nums text-muted">{formatMtime(node.mtime)}</div>
    </div>
  )
}
