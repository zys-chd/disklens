import { useScanStore } from '../store/scanStore'
import { backend } from '../lib/backend'
import { formatBytes, formatMtime } from '../lib/format'
import { ExternalIcon, FileIcon, FolderIcon } from './icons'
import { Empty } from './ui'

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

export function BottomPanel() {
  const topFiles = useScanStore((s) => s.topFiles)
  const topLimit = useScanStore((s) => s.topLimit)
  const setTopLimit = useScanStore((s) => s.setTopLimit)
  const reveal = useScanStore((s) => s.reveal)
  const status = useScanStore((s) => s.status)
  const notify = useScanStore((s) => s.notify)

  const open = async (path: string, show: boolean) => {
    try {
      await backend.openPath(path, show)
    } catch (e) {
      notify('error', errMsg(e))
    }
  }

  return (
    <section className="flex h-[38vh] min-h-[170px] shrink-0 flex-col border-t border-line bg-panel">
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-line px-3">
        <span className="text-[11px] font-semibold tracking-wide text-fg">Top 大文件</span>
        <span className="text-[10px] text-muted">{topFiles.length} 条 · 点击定位到树</span>
        <div className="flex-1" />
        <div className="flex items-center rounded-md border border-line p-0.5">
          {[50, 100, 500].map((n) => (
            <button
              key={n}
              type="button"
              onClick={() => void setTopLimit(n)}
              className={`rounded px-1.5 py-0.5 text-[10px] tabular-nums ${
                topLimit === n ? 'bg-sel font-semibold text-accent' : 'text-muted hover:text-fg'
              }`}
            >
              {n}
            </button>
          ))}
        </div>
      </div>

      {!topFiles.length ? (
        <Empty title={status === 'scanning' ? '扫描中…' : '暂无大文件'} hint={status === 'scanning' ? undefined : '完成扫描后展示 Top 文件'} />
      ) : (
        <div className="min-h-0 flex-1 overflow-auto">
          {topFiles.map((file, i) => (
            <div
              key={file.id}
              onClick={() => void reveal(file.path)}
              title={file.path}
              className="group grid h-7 cursor-pointer grid-cols-[34px_82px_minmax(0,1fr)_84px_56px] items-center gap-1 px-2 text-[11px] hover:bg-hover"
            >
              <span className="text-right tabular-nums text-muted/70">{i + 1}</span>
              <span className="text-right tabular-nums font-medium text-fg/90">{formatBytes(file.size)}</span>
              <span className="flex min-w-0 items-center gap-1.5">
                <FileIcon size={12} className="shrink-0 text-muted/70" />
                <span className="truncate font-mono text-[11px] text-fg/80">{file.path}</span>
              </span>
              <span className="text-right text-[10px] tabular-nums text-muted">{formatMtime(file.mtime)}</span>
              <span className="flex justify-end gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                <button
                  type="button"
                  title="打开文件"
                  aria-label="打开文件"
                  onClick={(e) => {
                    e.stopPropagation()
                    void open(file.path, false)
                  }}
                  className="rounded p-1 text-muted hover:bg-sel hover:text-accent"
                >
                  <ExternalIcon size={12} />
                </button>
                <button
                  type="button"
                  title="在文件管理器中显示"
                  aria-label="在文件管理器中显示"
                  onClick={(e) => {
                    e.stopPropagation()
                    void open(file.path, true)
                  }}
                  className="rounded p-1 text-muted hover:bg-sel hover:text-accent"
                >
                  <FolderIcon size={12} />
                </button>
              </span>
            </div>
          ))}
        </div>
      )}
    </section>
  )
}
