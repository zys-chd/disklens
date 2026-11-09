import { describe, expect, it } from 'vitest'
import { flattenRows, sortNodes } from './lib/tree'
import type { NodeView } from './lib/types'

function node(id: number, name: string, over: Partial<NodeView> = {}): NodeView {
  return {
    id,
    name,
    size: 100,
    ownSize: 0,
    files: 0,
    dirs: 0,
    mtime: 1000,
    kind: 'dir',
    hasChildren: false,
    errorCount: 0,
    ...over,
  }
}

const A = node(1, 'A', { hasChildren: true, size: 100 })
const A1 = node(2, 'A1', { hasChildren: true, size: 60 })
const A1a = node(3, 'a.txt', { kind: 'file', size: 30, files: 1 })
const A1b = node(4, 'b.txt', { kind: 'file', size: 30, files: 1 })
const A2 = node(5, 'A2', { kind: 'file', size: 40, files: 1 })
const B = node(6, 'B', { kind: 'file', size: 20, files: 1 })

const roots = [A, B]
const cache = new Map<number, NodeView[]>([
  [1, [A1, A2]],
  [2, [A1a, A1b]],
])
const getChildren = (id: number) => cache.get(id)

describe('flattenRows', () => {
  it('collapsed by default only shows roots', () => {
    expect(flattenRows(roots, getChildren, new Set()).map((r) => [r.node.name, r.depth])).toEqual([
      ['A', 0],
      ['B', 0],
    ])
  })

  it('expanded node inserts loaded children in order', () => {
    expect(flattenRows(roots, getChildren, new Set([1])).map((r) => [r.node.name, r.depth])).toEqual([
      ['A', 0],
      ['A1', 1],
      ['A2', 1],
      ['B', 0],
    ])
  })

  it('nested expansion keeps increasing depth', () => {
    const rows = flattenRows(roots, getChildren, new Set([1, 2]))
    expect(rows.map((r) => r.node.name)).toEqual(['A', 'A1', 'a.txt', 'b.txt', 'A2', 'B'])
    expect(rows.map((r) => r.depth)).toEqual([0, 1, 2, 2, 1, 0])
  })

  it('collapsing removes the whole loaded subtree', () => {
    const rows = flattenRows(roots, getChildren, new Set([2]))
    expect(rows.map((r) => r.node.name)).toEqual(['A', 'B'])
  })

  it('does not descend into an expanded node with no cache entry', () => {
    expect(flattenRows(roots, () => undefined, new Set([1])).map((r) => r.node.name)).toEqual(['A', 'B'])
  })
})

describe('sortNodes', () => {
  const items = [
    node(1, 'a', { size: 10, mtime: 200, files: 4 }),
    node(2, 'b', { size: 5, mtime: 100, files: 4 }),
    node(3, 'c', { size: 5, mtime: 100, files: 2 }),
    node(4, 'd', { size: 1, mtime: 300, files: 9 }),
  ]

  it('sorts size descending and keeps ties in insertion order', () => {
    expect(sortNodes(items, 'size', true).map((n) => n.id)).toEqual([1, 2, 3, 4])
  })

  it('sorts size ascending and keeps ties stable', () => {
    expect(sortNodes(items, 'size', false).map((n) => n.id)).toEqual([4, 2, 3, 1])
  })

  it('sorts mtime with stable ties', () => {
    expect(sortNodes(items, 'mtime', true).map((n) => n.id)).toEqual([4, 1, 2, 3])
  })

  it('sorts files and name', () => {
    expect(sortNodes(items, 'files', true).map((n) => n.id)).toEqual([4, 1, 2, 3])
    expect(sortNodes(items, 'name', false).map((n) => n.name)).toEqual(['a', 'b', 'c', 'd'])
  })
})
