import type { NodeView, SortBy } from './types'

export interface Row {
  node: NodeView
  depth: number
}

/** 把已加载的子节点按展开集拍平成可视行；未加载的目录即使展开也不产生子行。 */
export function flattenRows(
  roots: NodeView[],
  getChildren: (id: number) => NodeView[] | undefined,
  expanded: ReadonlySet<number>,
): Row[] {
  const rows: Row[] = []
  const seen = new Set<number>()
  const walk = (nodes: NodeView[], depth: number) => {
    for (const node of nodes) {
      if (seen.has(node.id)) continue // 防御异常数据成环
      seen.add(node.id)
      rows.push({ node, depth })
      if (node.hasChildren && expanded.has(node.id)) {
        const children = getChildren(node.id)
        if (children) walk(children, depth + 1)
      }
    }
  }
  walk(roots, 0)
  return rows
}

/** 排序比较器；相等时按 id 保证稳定（与后端 query.rs 同规则）。 */
export function compareNodes(a: NodeView, b: NodeView, sortBy: SortBy, desc: boolean): number {
  let d = 0
  switch (sortBy) {
    case 'size':
      d = a.size - b.size
      break
    case 'files':
      d = a.files - b.files
      break
    case 'mtime':
      d = a.mtime - b.mtime
      break
    case 'name':
      d = a.name.localeCompare(b.name)
      break
  }
  if (d === 0) return a.id - b.id
  return desc ? -d : d
}

export function sortNodes(nodes: NodeView[], sortBy: SortBy, desc: boolean): NodeView[] {
  return [...nodes].sort((a, b) => compareNodes(a, b, sortBy, desc))
}

export function childKey(
  nodeId: number,
  sortBy: SortBy,
  desc: boolean,
  filter: string,
  minSize: number,
  onlySafe = false,
): string {
  return `${nodeId}:${sortBy}:${desc}:${filter}:${minSize}:${onlySafe}`
}
