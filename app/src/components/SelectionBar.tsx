import { useScanStore } from '../store/scanStore'
import { formatBytes } from '../lib/format'
import { TrashIcon, XIcon } from './icons'
import { Button } from './ui'

export function SelectionBar() {
  const selection = useScanStore((s) => s.selection)
  const requestDelete = useScanStore((s) => s.requestDelete)
  const clearSelection = useScanStore((s) => s.clearSelection)

  const items = Object.values(selection)
  if (!items.length) return null
  const total = items.reduce((acc, i) => acc + i.size, 0)
  const canPermanent = items.length === 1

  return (
    <div
      className="animate-in fixed right-4 z-30 flex items-center gap-3 rounded-lg border border-line bg-panel/95 py-1.5 pl-3 pr-1.5 text-xs shadow-2xl backdrop-blur"
      style={{ bottom: 'calc(38vh + 12px)' }}
    >
      <span className="whitespace-nowrap text-muted">
        已选 <b className="font-semibold tabular-nums text-fg">{items.length}</b> 项 · 合计{' '}
        <b className="font-semibold tabular-nums text-fg">{formatBytes(total)}</b>
      </span>
      <Button variant="default" onClick={() => void requestDelete('trash')}>
        <TrashIcon size={12} />
        移入回收站
      </Button>
      <Button
        variant="danger"
        disabled={!canPermanent}
        title={canPermanent ? '永久删除需输入名称确认' : '永久删除一次只支持一项，多项请用回收站'}
        onClick={() => void requestDelete('permanent')}
      >
        永久删除
      </Button>
      <button
        type="button"
        aria-label="清除选择"
        title="清除选择"
        onClick={clearSelection}
        className="grid h-6 w-6 place-items-center rounded text-muted hover:bg-hover hover:text-fg"
      >
        <XIcon size={13} />
      </button>
    </div>
  )
}
