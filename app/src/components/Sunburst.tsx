import { useEffect, useMemo, useState } from 'react'
import { hierarchy, partition, type HierarchyRectangularNode } from 'd3-hierarchy'
import { arc } from 'd3-shape'
import { useScanStore } from '../store/scanStore'
import type { NodeView } from '../lib/types'
import { formatBytes, percent } from '../lib/format'
import { useBox } from './useBox'

interface SunNode {
  id: number
  name: string
  size: number
  node: NodeView
  children?: SunNode[]
}

function buildSun(
  node: NodeView,
  depth: number,
  children: Record<string, NodeView[]>,
  keyFor: (id: number) => string,
): SunNode {
  const kids = depth < 2 ? (children[keyFor(node.id)] ?? []).slice(0, 120) : []
  return {
    id: node.id,
    name: node.name,
    size: node.size,
    node,
    children: kids.length ? kids.map((k) => buildSun(k, depth + 1, children, keyFor)) : undefined,
  }
}

const colorOf = (depth: number, id: number) => {
  const hue = 244 + (id % 5) * 5
  const light = 62 - depth * 7
  return `hsl(${hue} 66% ${light}%)`
}

export function Sunburst() {
  const chain = useScanStore((s) => s.chain)
  const children = useScanStore((s) => s.children)
  const keyFor = useScanStore((s) => s.keyFor)
  const ensureChildren = useScanStore((s) => s.ensureChildren)
  const focusNode = useScanStore((s) => s.focusNode)
  const sortBy = useScanStore((s) => s.sortBy)
  const desc = useScanStore((s) => s.desc)
  const filter = useScanStore((s) => s.filter)
  const minSize = useScanStore((s) => s.minSize)

  const focus = chain[chain.length - 1]
  const { ref, box } = useBox<HTMLDivElement>()
  const [hoverId, setHoverId] = useState<number | null>(null)

  useEffect(() => {
    if (!focus?.hasChildren) return
    let cancelled = false
    void (async () => {
      await ensureChildren(focus.id)
      if (cancelled) return
      const kids = useScanStore.getState().children[keyFor(focus.id)] ?? []
      await Promise.all(
        kids
          .filter((k) => k.kind === 'dir' && k.hasChildren)
          .slice(0, 10)
          .map((k) => ensureChildren(k.id)),
      )
    })()
    return () => {
      cancelled = true
    }
  }, [focus?.id, focus?.hasChildren, ensureChildren, keyFor, sortBy, desc, filter, minSize])

  const data = useMemo(
    () => (focus ? buildSun(focus, 0, children, keyFor) : null),
    [focus, children, keyFor],
  )

  const radius = Math.max(10, Math.min(box.width, box.height) / 2 - 10)

  const arcs = useMemo(() => {
    if (!data || radius <= 10) return []
    const root = hierarchy(data, (d) => d.children).sum((d) =>
      d.children?.length ? 0 : Math.max(d.size, 1),
    )
    const parts = partition<SunNode>().size([Math.PI * 2, radius])(root)
    const gen = arc<HierarchyRectangularNode<SunNode>>()
      .startAngle((d) => d.x0)
      .endAngle((d) => d.x1)
      .padAngle(0.002)
      .innerRadius((d) => d.y0)
      .outerRadius((d) => d.y1)
    return parts
      .descendants()
      .filter((d) => d.depth > 0 && d.depth <= 2)
      .map((d) => ({ d, path: gen(d) ?? '' }))
  }, [data, radius])

  const nodeById = useMemo(() => {
    const map = new Map<number, NodeView>()
    if (!data) return map
    const walk = (n: SunNode) => {
      map.set(n.id, n.node)
      n.children?.forEach(walk)
    }
    walk(data)
    return map
  }, [data])

  if (!focus) return <div className="grid h-full place-items-center text-xs text-muted">暂无数据</div>

  return (
    <div ref={ref} className="relative grid h-full w-full place-items-center overflow-hidden bg-panel">
      {box.width > 0 && (
        <svg width={box.width} height={box.height} viewBox={`0 0 ${box.width} ${box.height}`}>
          <g transform={`translate(${box.width / 2},${box.height / 2})`}>
            {arcs.map(({ d, path }) => {
              const node = nodeById.get(d.data.id)
              if (!node) return null
              const hovered = hoverId === node.id
              return (
                <path
                  key={node.id}
                  d={path}
                  fill={colorOf(d.depth, node.id)}
                  fillOpacity={hovered ? 1 : 0.88}
                  stroke="var(--panel)"
                  strokeWidth={0.8}
                  className="cursor-pointer transition-[fill-opacity]"
                  onMouseEnter={() => setHoverId(node.id)}
                  onMouseLeave={() => setHoverId(null)}
                  onClick={() => void focusNode(node, true)}
                >
                  <title>{`${node.name} · ${formatBytes(node.size)}`}</title>
                </path>
              )
            })}
            <text textAnchor="middle" y={-6} className="fill-fg text-[11px] font-medium">
              {focus.name.length > 18 ? `${focus.name.slice(0, 18)}…` : focus.name}
            </text>
            <text textAnchor="middle" y={10} className="fill-muted text-[10px] tabular-nums">
              {formatBytes(focus.size)}
            </text>
          </g>
        </svg>
      )}
      {hoverId != null && nodeById.get(hoverId) && (
        <div className="pointer-events-none absolute left-2 top-2 rounded-md border border-line bg-panel/95 px-2 py-1 text-[11px] shadow-sm">
          <span className="font-medium">{nodeById.get(hoverId)!.name}</span>
          <span className="ml-1.5 tabular-nums text-muted">
            {formatBytes(nodeById.get(hoverId)!.size)} ·{' '}
            {percent(nodeById.get(hoverId)!.size, focus.size).toFixed(1)}%
          </span>
        </div>
      )}
    </div>
  )
}
