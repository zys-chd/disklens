import { describe, expect, it } from 'vitest'
import { layoutTreemap, toTreemapInput } from './lib/treemap'

describe('toTreemapInput', () => {
  it('maps only the supplied children into hierarchy input', () => {
    const input = toTreemapInput(
      { id: 0, name: 'root', size: 1000, kind: 'dir' },
      [
        { id: 1, name: 'big', size: 600, kind: 'dir' },
        { id: 2, name: 'mid', size: 300, kind: 'file' },
        { id: 3, name: 'small', size: 100, kind: 'file' },
      ],
    )
    expect(input.id).toBe(0)
    expect(input.children).toHaveLength(3)
    expect(input.children?.[0]).toEqual({ id: 1, name: 'big', size: 600, kind: 'dir' })
    expect(input.children?.reduce((a, c) => a + c.size, 0)).toBe(1000)
  })
})

describe('layoutTreemap', () => {
  const input = toTreemapInput(
    { id: 0, name: 'root', size: 1000, kind: 'dir' },
    [
      { id: 1, name: 'big', size: 600, kind: 'dir' },
      { id: 2, name: 'mid', size: 300, kind: 'file' },
      { id: 3, name: 'small', size: 100, kind: 'file' },
    ],
  )

  it('returns one rectangle per child, excluding the root', () => {
    const rects = layoutTreemap(input, 1000, 1000)
    expect(rects.map((r) => r.id)).toEqual([1, 2, 3])
    expect(rects.every((r) => r.width > 0 && r.height > 0)).toBe(true)
  })

  it('keeps every rectangle inside the canvas', () => {
    const rects = layoutTreemap(input, 800, 600)
    for (const r of rects) {
      expect(r.x).toBeGreaterThanOrEqual(0)
      expect(r.y).toBeGreaterThanOrEqual(0)
      expect(r.x + r.width).toBeLessThanOrEqual(800)
      expect(r.y + r.height).toBeLessThanOrEqual(600)
    }
  })

  it('area follows the byte ratio (big ≈ 6x small)', () => {
    const rects = layoutTreemap(input, 1000, 1000)
    const area = (id: number) => {
      const r = rects.find((x) => x.id === id)!
      return r.width * r.height
    }
    expect(area(1)).toBeGreaterThan(area(2) * 1.8)
    expect(area(2)).toBeGreaterThan(area(3) * 2.5)
    expect(area(1) / area(3)).toBeGreaterThan(5)
  })
})
