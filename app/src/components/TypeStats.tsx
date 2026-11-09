import { useMemo } from 'react'
import { useScanStore } from '../store/scanStore'
import { formatBytes, formatCount, percent } from '../lib/format'
import { Empty } from './ui'

const OTHER = '__other__'

export function TypeStats() {
  const typeStats = useScanStore((s) => s.typeStats)
  const summary = useScanStore((s) => s.summary)

  const total = summary?.root.size ?? 0
  const rows = useMemo(() => {
    const top = typeStats.slice(0, 12)
    const sum = top.reduce((acc, r) => acc + r.bytes, 0)
    const other = Math.max(0, total - sum)
    return other > 0
      ? [
          ...top,
          {
            ext: OTHER,
            bytes: other,
            files: Math.max(0, summary ? summary.root.files - top.reduce((a, r) => a + r.files, 0) : 0),
          },
        ]
      : top
  }, [typeStats, total, summary])

  if (!rows.length) return <Empty title="暂无类型数据" hint="扫描完成后展示" />

  const max = Math.max(...rows.map((r) => r.bytes), 1)

  return (
    <div className="flex h-full flex-col">
      <div className="min-h-0 flex-1 overflow-auto px-3 py-2">
        {rows.map((row) => {
          const isOther = row.ext === OTHER
          const label = isOther ? '其他' : row.ext ? `.${row.ext}` : '无扩展名'
          return (
            <div
              key={row.ext || 'none'}
              className="mb-0.5 grid w-full grid-cols-[86px_minmax(0,1fr)_52px_74px] items-center gap-2 px-1 py-[3px] text-left"
              title={`${label} · ${formatCount(row.files)} 个文件`}
            >
              <span className="truncate text-[11px] text-fg/85">{label}</span>
              <span className="relative h-4 overflow-hidden rounded-sm bg-line/50">
                <span
                  className={`absolute inset-y-0 left-0 rounded-sm ${isOther ? 'bg-muted/45' : 'bg-accent/80'}`}
                  style={{ width: `${Math.max(1, percent(row.bytes, max))}%` }}
                />
              </span>
              <span className="text-right text-[11px] tabular-nums text-fg/85">{formatBytes(row.bytes)}</span>
              <span className="text-right text-[10px] tabular-nums text-muted">{formatCount(row.files)} 个</span>
            </div>
          )
        })}
      </div>
      <div className="shrink-0 border-t border-line px-3 py-1.5 text-[10px] text-muted">
        共 {formatBytes(total)} · Top 12 扩展名，其余合并为「其他」
      </div>
    </div>
  )
}
