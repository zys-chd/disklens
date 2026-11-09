import { useScanStore } from '../store/scanStore'
import { formatBytes, formatCount } from '../lib/format'
import { Button } from './ui'

export function ScanProgress() {
  const progress = useScanStore((s) => s.progress)
  const cancelScan = useScanStore((s) => s.cancelScan)
  const rootPath = useScanStore((s) => s.rootPath)

  return (
    <div className="flex h-8 shrink-0 items-center gap-3 border-b border-line bg-panel2 px-3 text-[11px]">
      <div className="relative h-1 w-36 shrink-0 overflow-hidden rounded-full bg-line">
        <div className="progress-stripe absolute inset-y-0 left-0 w-2/5 rounded-full bg-accent" />
      </div>
      <span className="shrink-0 tabular-nums text-fg">
        已扫 {formatCount(progress?.entries ?? 0)} 项
      </span>
      <span className="shrink-0 tabular-nums text-muted">{formatBytes(progress?.bytes ?? 0)}</span>
      <span className="min-w-0 flex-1 truncate font-mono text-muted" title={progress?.currentPath ?? rootPath}>
        {progress?.currentPath ?? rootPath}
      </span>
      <span className="shrink-0 tabular-nums text-muted">
        {((progress?.elapsedMs ?? 0) / 1000).toFixed(1)}s
      </span>
      <Button variant="default" onClick={() => void cancelScan()}>
        停止
      </Button>
    </div>
  )
}
