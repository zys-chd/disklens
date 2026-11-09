import { useScanStore } from '../store/scanStore'
import { ArrowUpIcon, ChevronRight, HomeIcon } from './icons'

export function Breadcrumb() {
  const chain = useScanStore((s) => s.chain)
  const rootPath = useScanStore((s) => s.rootPath)
  const status = useScanStore((s) => s.status)
  const focusNode = useScanStore((s) => s.focusNode)

  const root = chain[0]
  const focus = chain[chain.length - 1]
  const parent = chain[chain.length - 2] ?? root

  return (
    <nav className="flex h-8 shrink-0 items-center gap-1 border-b border-line bg-panel px-2 text-xs">
      <button
        type="button"
        aria-label="回到根目录"
        title="回到根目录"
        disabled={!root}
        onClick={() => root && void focusNode(root, true)}
        className="rounded p-1 text-muted hover:bg-hover hover:text-fg disabled:opacity-40"
      >
        <HomeIcon size={13} />
      </button>
      <button
        type="button"
        aria-label="上一级"
        title="上一级"
        disabled={!parent || focus?.id === root?.id}
        onClick={() => parent && void focusNode(parent, true)}
        className="rounded p-1 text-muted hover:bg-hover hover:text-fg disabled:opacity-40"
      >
        <ArrowUpIcon size={13} />
      </button>

      <div className="flex min-w-0 items-center gap-0.5 overflow-hidden">
        <button
          type="button"
          disabled={!root}
          onClick={() => root && void focusNode(root, true)}
          className="truncate rounded px-1 py-0.5 font-mono text-[11px] text-muted hover:bg-hover hover:text-fg"
          title={rootPath}
        >
          {rootPath || '—'}
        </button>
        {chain.slice(1).map((node, i) => (
          <span key={node.id} className="flex min-w-0 items-center">
            <ChevronRight size={11} className="shrink-0 text-muted/60" />
            <button
              type="button"
              onClick={() => void focusNode(node, true, chain.slice(0, i + 2))}
              className={`max-w-[220px] truncate rounded px-1 py-0.5 text-[12px] hover:bg-hover ${
                i === chain.length - 2 ? 'font-medium text-fg' : 'text-muted hover:text-fg'
              }`}
              title={node.name}
            >
              {node.name}
            </button>
          </span>
        ))}
      </div>

      <div className="flex-1" />
      {status === 'done' && focus && focus.errorCount > 0 && (
        <span className="shrink-0 rounded bg-warn/10 px-1.5 py-0.5 text-[10px] text-warn">
          {focus.errorCount} 个条目不可读
        </span>
      )}
    </nav>
  )
}
