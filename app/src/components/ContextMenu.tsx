import { useEffect } from 'react'
import { useScanStore } from '../store/scanStore'
import { backend } from '../lib/backend'
import { CopyIcon, ExternalIcon, FolderIcon } from './icons'

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

export function ContextMenu() {
  const menu = useScanStore((s) => s.contextMenu)
  const scanId = useScanStore((s) => s.scanId)
  const close = useScanStore((s) => s.closeContextMenu)
  const notify = useScanStore((s) => s.notify)

  useEffect(() => {
    if (!menu) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close()
    }
    const onScroll = () => close()
    window.addEventListener('keydown', onKey)
    window.addEventListener('scroll', onScroll, true)
    window.addEventListener('resize', onScroll)
    window.addEventListener('click', close)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('scroll', onScroll, true)
      window.removeEventListener('resize', onScroll)
      window.removeEventListener('click', close)
    }
  }, [menu, close])

  if (!menu) return null
  const { node, x, y } = menu

  const withPath = async (fn: (path: string) => Promise<void>) => {
    if (!scanId) return
    try {
      await fn(await backend.nodePath(scanId, node.id))
      close()
    } catch (e) {
      notify('error', errMsg(e))
      close()
    }
  }

  const copy = async (text: string) => {
    try {
      if (!navigator.clipboard) throw new Error('剪贴板不可用')
      await navigator.clipboard.writeText(text)
      notify('success', '已复制')
    } catch (e) {
      notify('error', errMsg(e))
    }
    close()
  }

  const itemClass =
    'flex w-full items-center gap-2 rounded-[5px] px-2 py-1.5 text-left text-xs text-fg/90 hover:bg-hover'

  return (
    <div
      role="menu"
      className="animate-in fixed z-40 w-[210px] rounded-md border border-line bg-panel p-1 shadow-2xl"
      style={{ left: Math.min(x, window.innerWidth - 220), top: Math.min(y, window.innerHeight - 200) }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="truncate px-2 pb-1 pt-0.5 text-[10px] text-muted">{node.name}</div>
      <button
        type="button"
        className={itemClass}
        onClick={() => void withPath((p) => backend.openPath(p, false))}
      >
        <ExternalIcon size={13} className="text-muted" />
        打开文件
      </button>
      <button
        type="button"
        className={itemClass}
        onClick={() => void withPath((p) => backend.openPath(p, true))}
      >
        <FolderIcon size={13} className="text-muted" />
        在文件管理器中显示
      </button>
      <div className="my-1 h-px bg-line" />
      <button type="button" className={itemClass} onClick={() => void withPath((p) => copy(p))}>
        <CopyIcon size={13} className="text-muted" />
        复制路径
      </button>
      <button type="button" className={itemClass} onClick={() => void copy(node.name)}>
        <CopyIcon size={13} className="text-muted" />
        复制名称
      </button>
      {node.safetyCommand && (
        <button
          type="button"
          className={itemClass}
          title={node.safetyCommand}
          onClick={() => void copy(node.safetyCommand ?? '')}
        >
          <CopyIcon size={13} className="text-muted" />
          复制建议命令
        </button>
      )}
    </div>
  )
}
