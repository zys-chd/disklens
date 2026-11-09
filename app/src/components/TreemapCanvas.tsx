import { useEffect, useMemo, useRef, useState } from 'react'
import { useScanStore } from '../store/scanStore'
import { layoutTreemap, toTreemapInput, type TreemapRect } from '../lib/treemap'
import { formatBytes, formatCount, percent } from '../lib/format'
import { Spinner } from './ui'
import { useBox } from './useBox'

const PALETTE = ['#6366f1', '#5b5bd6', '#7c7ff2', '#4f46e5', '#818cf8', '#6d6af0', '#8b8df4']

function colorOf(rect: TreemapRect): string {
  return PALETTE[Math.abs(rect.id * 7 + rect.depth) % PALETTE.length]
}

export function TreemapCanvas() {
  const chain = useScanStore((s) => s.chain)
  const children = useScanStore((s) => s.children)
  const loading = useScanStore((s) => s.loading)
  const keyFor = useScanStore((s) => s.keyFor)
  const ensureChildren = useScanStore((s) => s.ensureChildren)
  const focusNode = useScanStore((s) => s.focusNode)
  const selectNode = useScanStore((s) => s.selectNode)

  const focus = chain[chain.length - 1]
  const { ref, box } = useBox<HTMLDivElement>()
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [hoverId, setHoverId] = useState<number | null>(null)
  const [pos, setPos] = useState({ x: 0, y: 0 })

  const items = focus ? (children[keyFor(focus.id)] ?? []) : []
  const loadingNow = focus ? loading.has(keyFor(focus.id)) : false

  useEffect(() => {
    if (focus?.hasChildren) void ensureChildren(focus.id)
  }, [focus?.id, focus?.hasChildren, ensureChildren])

  const rects = useMemo(
    () =>
      focus && box.width > 0 && box.height > 0
        ? layoutTreemap(toTreemapInput(focus, items), box.width, box.height)
        : [],
    [focus, items, box.width, box.height],
  )

  const byId = useMemo(() => new Map(items.map((n) => [n.id, n])), [items])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || box.width === 0 || box.height === 0) return
    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.max(1, Math.round(box.width * dpr))
    canvas.height = Math.max(1, Math.round(box.height * dpr))
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, box.width, box.height)

    for (const rect of rects) {
      if (rect.width < 1 || rect.height < 1) continue
      ctx.fillStyle = colorOf(rect)
      ctx.fillRect(rect.x, rect.y, rect.width, rect.height)
      ctx.strokeStyle = 'rgba(255,255,255,0.12)'
      ctx.lineWidth = 1
      ctx.strokeRect(rect.x + 0.5, rect.y + 0.5, Math.max(0, rect.width - 1), Math.max(0, rect.height - 1))

      if (rect.width > 52 && rect.height > 18) {
        ctx.save()
        ctx.beginPath()
        ctx.rect(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2)
        ctx.clip()
        ctx.fillStyle = 'rgba(255,255,255,0.96)'
        ctx.font = '600 11px ui-sans-serif, system-ui, sans-serif'
        ctx.fillText(rect.name, rect.x + 5, rect.y + 15, Math.max(0, rect.width - 10))
        if (rect.height > 34) {
          ctx.fillStyle = 'rgba(255,255,255,0.72)'
          ctx.font = '10px ui-monospace, monospace'
          ctx.fillText(formatBytes(rect.size), rect.x + 5, rect.y + 29, Math.max(0, rect.width - 10))
        }
        ctx.restore()
      }

      if (rect.id === hoverId) {
        ctx.fillStyle = 'rgba(255,255,255,0.18)'
        ctx.fillRect(rect.x, rect.y, rect.width, rect.height)
        ctx.strokeStyle = '#c7d2fe'
        ctx.lineWidth = 1.5
        ctx.strokeRect(rect.x + 0.75, rect.y + 0.75, Math.max(0, rect.width - 1.5), Math.max(0, rect.height - 1.5))
      }
    }
  }, [rects, box.width, box.height, hoverId])

  const hitTest = (clientX: number, clientY: number): TreemapRect | null => {
    const canvas = canvasRef.current
    if (!canvas) return null
    const r = canvas.getBoundingClientRect()
    const x = clientX - r.left
    const y = clientY - r.top
    for (let i = rects.length - 1; i >= 0; i--) {
      const rect = rects[i]
      if (x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height) return rect
    }
    return null
  }

  const onMove = (e: React.MouseEvent) => {
    const rect = hitTest(e.clientX, e.clientY)
    setHoverId(rect?.id ?? null)
    setPos({ x: e.clientX - (canvasRef.current?.getBoundingClientRect().left ?? 0), y: e.clientY - (canvasRef.current?.getBoundingClientRect().top ?? 0) })
  }

  const onClick = (e: React.MouseEvent) => {
    const rect = hitTest(e.clientX, e.clientY)
    const node = rect ? byId.get(rect.id) : undefined
    if (!node) return
    if (node.kind === 'dir') {
      void focusNode(node, true, [...chain, node])
    } else {
      selectNode(node, 'single', [])
      useScanStore.setState({ scrollTarget: node.id })
    }
  }

  const hoverNode = hoverId != null ? byId.get(hoverId) : undefined

  return (
    <div ref={ref} className="relative h-full w-full overflow-hidden bg-panel">
      <canvas
        ref={canvasRef}
        className="block h-full w-full cursor-pointer"
        style={{ width: box.width, height: box.height }}
        onMouseMove={onMove}
        onMouseLeave={() => setHoverId(null)}
        onClick={onClick}
      />
      {focus && (
        <div className="pointer-events-none absolute left-2 top-2 max-w-[70%] truncate rounded bg-panel/85 px-1.5 py-0.5 text-[10px] text-muted shadow-sm backdrop-blur">
          {chain.map((n) => n.name).join(' / ')} · {formatBytes(focus.size)}
        </div>
      )}
      {loadingNow && items.length === 0 ? (
        <div className="absolute inset-0 grid place-items-center bg-panel/60">
          <Spinner />
        </div>
      ) : items.length === 0 ? (
        <div className="absolute inset-0 grid place-items-center p-6 text-center text-xs text-muted">
          {focus?.kind === 'file' ? '文件没有子项；双击文件夹下钻' : '此目录没有可展示的子项'}
        </div>
      ) : null}

      {hoverNode && (
        <div
          className="pointer-events-none absolute z-20 min-w-[150px] max-w-[260px] rounded-md border border-line bg-panel/95 px-2 py-1.5 text-[11px] shadow-lg backdrop-blur"
          style={{
            left: Math.min(pos.x + 12, Math.max(0, box.width - 220)),
            top: Math.min(pos.y + 16, Math.max(0, box.height - 78)),
          }}
        >
          <div className="truncate font-medium text-fg">{hoverNode.name}</div>
          <div className="mt-0.5 tabular-nums text-muted">
            {formatBytes(hoverNode.size)} · {percent(hoverNode.size, focus?.size ?? 0).toFixed(1)}%
          </div>
          <div className="tabular-nums text-muted">
            {formatCount(hoverNode.files)} 个文件 · {hoverNode.kind === 'dir' ? '目录' : '文件'}
          </div>
        </div>
      )}
    </div>
  )
}
