import { hierarchy, treemap } from 'd3-hierarchy'
import type { NodeKind } from './types'

export interface TreemapInput {
  id: number
  name: string
  size: number
  kind: NodeKind
  children?: TreemapInput[]
}

export interface TreemapRect {
  id: number
  name: string
  size: number
  kind: NodeKind
  depth: number
  x: number
  y: number
  width: number
  height: number
}

export function toTreemapInput(
  root: { id: number; name: string; size: number; kind: NodeKind },
  children: { id: number; name: string; size: number; kind: NodeKind }[],
): TreemapInput {
  return {
    id: root.id,
    name: root.name,
    size: root.size,
    kind: root.kind,
    children: children.map((c) => ({ id: c.id, name: c.name, size: c.size, kind: c.kind })),
  }
}

/** d3.treemap 算布局；返回当前节点下一层的矩形（组件只画一层）。 */
export function layoutTreemap(input: TreemapInput, width: number, height: number): TreemapRect[] {
  const w = Math.max(1, Math.floor(width))
  const h = Math.max(1, Math.floor(height))
  const root = hierarchy(input, (d) => d.children).sum((d) =>
    d.children?.length ? 0 : Math.max(d.size, 1),
  )
  const laid = treemap<TreemapInput>()
    .size([w, h])
    .paddingInner(1)
    .paddingOuter(2)
    .round(true)(root)
  return laid
    .descendants()
    .filter((d) => d.depth === 1)
    .map((d) => ({
      id: d.data.id,
      name: d.data.name,
      size: d.data.size,
      kind: d.data.kind,
      depth: d.depth,
      x: d.x0,
      y: d.y0,
      width: Math.max(0, d.x1 - d.x0),
      height: Math.max(0, d.y1 - d.y0),
    }))
}
