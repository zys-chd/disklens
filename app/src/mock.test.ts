import { beforeAll, describe, expect, it } from 'vitest'
import { MockBackend } from './lib/backend/mock'
import type { NodeView, ScanProgress, ScanSummary } from './lib/types'

const GB = 1024 ** 3
const MB = 1024 ** 2

const backend = new MockBackend({ scanMs: 40 })
let scanId = ''

beforeAll(async () => {
  scanId = (await backend.startScan('/home/zys')).scanId
})

const list = (nodeId: number, over: Partial<Parameters<MockBackend['listChildren']>[0]> = {}) =>
  backend.listChildren({
    scanId,
    nodeId,
    sortBy: 'size',
    desc: true,
    offset: 0,
    limit: 5000,
    ...over,
  })

describe('mock tree shape', () => {
  it('root has the expected top-level layout', async () => {
    const root = await backend.getNode(scanId, 0)
    expect(root.id).toBe(0)
    expect(root.kind).toBe('dir')
    expect(root.size).toBeGreaterThan(50 * GB)
    expect(root.files).toBeGreaterThan(18000)
    const names = (await list(0)).items.map((n) => n.name)
    expect(names).toEqual([
      '.local',
      'Downloads',
      'Videos',
      'Projects',
      '.cache',
      'Music',
      'Pictures',
      'Documents',
      '.hermes',
      '.config',
      'notes.md',
      '.ssh',
      'latest-backup',
    ])
  })

  it('builds exactly ~20k deterministic nodes', async () => {
    let id = ''
    const done = new Promise<ScanSummary>((resolve) => {
      const off = backend.onDone((s) => {
        if (s.scanId !== id) return
        off()
        resolve(s)
      })
    })
    id = (await backend.startScan('/home/zys')).scanId
    const summary = await done
    expect(summary.scanId).toBe(id)
    expect(summary.entries).toBe(20000)
    expect(summary.cancelled).toBe(false)
    expect(summary.root.size).toBeGreaterThan(50 * GB)
  })
})

describe('listChildren sorting and pagination', () => {
  it('size desc is monotonic and paginates without gaps', async () => {
    const full = await list(0)
    expect(full.total).toBe(13)
    for (let i = 1; i < full.items.length; i++) {
      expect(full.items[i - 1].size).toBeGreaterThanOrEqual(full.items[i].size)
    }
    const page1 = await list(0, { limit: 3, offset: 0 })
    const page2 = await list(0, { limit: 4, offset: 3 })
    expect(page1.total).toBe(13)
    expect(page2.total).toBe(13)
    expect([...page1.items, ...page2.items].map((n) => n.id)).toEqual(
      full.items.slice(0, 7).map((n) => n.id),
    )
  })

  it('offset past total returns empty items with real total', async () => {
    const page = await list(0, { offset: 500, limit: 10 })
    expect(page.total).toBe(13)
    expect(page.items).toEqual([])
  })

  it('name asc sorts deterministically', async () => {
    const res = await list(0, { sortBy: 'name', desc: false })
    expect(res.items[0].name).toBe('.cache')
    const names = res.items.map((n) => n.name)
    expect(names).toEqual([...names].sort((a, b) => a.localeCompare(b)))
  })

  it('files and mtime sorting are consistent', async () => {
    const byFiles = await list(0, { sortBy: 'files', desc: true })
    for (let i = 1; i < byFiles.items.length; i++) {
      expect(byFiles.items[i - 1].files).toBeGreaterThanOrEqual(byFiles.items[i].files)
    }
    const byTime = await list(0, { sortBy: 'mtime', desc: false })
    for (let i = 1; i < byTime.items.length; i++) {
      expect(byTime.items[i - 1].mtime).toBeLessThanOrEqual(byTime.items[i].mtime)
    }
  })

  it('filter is case-insensitive substring on name only', async () => {
    const projects = (await list(0, { filter: 'projects' })).items
    expect(projects.map((n) => n.name)).toEqual(['Projects'])
    const kids = await list(projects[0].id, { filter: 'DISKLENS' })
    expect(kids.total).toBe(1)
    expect(kids.items[0].name).toBe('disklens')
    expect(kids.items[0].kind).toBe('dir')
  })

  it('minSize filters on aggregated size', async () => {
    const all = await list(0)
    const minSize = 4 * GB
    const filtered = await list(0, { minSize })
    const expected: NodeView[] = all.items.filter((n) => n.size >= minSize)
    expect(filtered.total).toBe(expected.length)
    expect(filtered.items.map((n) => n.id)).toEqual(expected.map((n) => n.id))
  })
})

describe('top files and type stats', () => {
  it('top files are globally size-ordered', async () => {
    const rows = await backend.topFiles(scanId, 50)
    expect(rows).toHaveLength(50)
    expect(rows[0].name).toBe('large.bin')
    expect(rows[0].size).toBe(6 * GB + 720 * MB)
    expect(rows[0].path).toBe('/home/zys/Downloads/large.bin')
    for (let i = 1; i < rows.length; i++) {
      expect(rows[i - 1].size).toBeGreaterThanOrEqual(rows[i].size)
    }
  })

  it('type stats aggregate by extension', async () => {
    const stats = await backend.typeStats(scanId, 12)
    expect(stats).toHaveLength(12)
    const iso = stats.find((s) => s.ext === 'iso')
    expect(iso).toEqual({ ext: 'iso', bytes: 3 * GB + 100 * MB, files: 1 })
    const pak = stats.find((s) => s.ext === 'pak')
    expect(pak?.files).toBe(45 + 38 + 22 + 18)
    for (let i = 1; i < stats.length; i++) {
      expect(stats[i - 1].bytes).toBeGreaterThanOrEqual(stats[i].bytes)
    }
  })
})

describe('delete simulation', () => {
  it('reports per-item success, repeat failure and guard rejections', async () => {
    const b = new MockBackend({ scanMs: 40 })
    const { scanId: id } = await b.startScan('/home/zys')
    const large = '/home/zys/Downloads/large.bin'

    expect(await b.moveToTrash([large])).toEqual({ ok: [large], failed: [] })
    expect(await b.moveToTrash([large])).toEqual({
      ok: [],
      failed: [{ path: large, error: '文件不存在' }],
    })
    expect(await b.moveToTrash(['/usr'])).toEqual({
      ok: [],
      failed: [{ path: '/usr', error: '系统路径受保护' }],
    })
    expect(await b.moveToTrash(['/home/zys'])).toEqual({
      ok: [],
      failed: [{ path: '/home/zys', error: '受保护目录，禁止删除' }],
    })
    expect(await b.moveToTrash(['/home/zys/.cache/locked/locked.db'])).toEqual({
      ok: [],
      failed: [{ path: '/home/zys/.cache/locked/locked.db', error: '文件被占用（模拟）' }],
    })

    const iso = '/home/zys/Downloads/ubuntu-24.04.1-desktop-amd64.iso'
    expect(await b.deletePermanent([iso], 'wrong')).toEqual({
      ok: [],
      failed: [{ path: iso, error: '确认文本与目录名不一致' }],
    })
    expect(await b.deletePermanent([iso], 'ubuntu-24.04.1-desktop-amd64.iso')).toEqual({
      ok: [iso],
      failed: [],
    })
    // 超时保护：扫描未启动时也不会挂住
    expect(await b.listSnapshots()).toEqual([])
    expect(id).toMatch(/^mock-\d+$/)
  })
})

describe('scan progress events', () => {
  it('emits progress ticks then a summary with 20000 entries', async () => {
    const b = new MockBackend({ scanMs: 40 })
    const progress: ScanProgress[] = []
    const done = new Promise<ScanSummary>((resolve) => {
      b.onDone((s) => resolve(s))
    })
    b.onProgress((p) => progress.push(p))
    await b.startScan('/home/zys')
    const summary = await done

    expect(progress).toHaveLength(4)
    expect(progress[0].entries).toBe(5000)
    expect(progress[0].bytes).toBeGreaterThan(0)
    expect(progress[0].currentPath.startsWith('/home/zys/')).toBe(true)
    expect(progress[3].entries).toBe(19400)
    expect(progress[3].elapsedMs).toBeGreaterThan(0)
    expect(summary.entries).toBe(20000)
    expect(summary.errors).toBe(3)
  })

  it('cancel keeps partial entries and flags cancelled', async () => {
    const b = new MockBackend({ scanMs: 2000 })
    const done = new Promise<ScanSummary>((resolve) => {
      b.onDone((s) => resolve(s))
    })
    const { scanId: id } = await b.startScan('/home/zys')
    await new Promise((r) => setTimeout(r, 30))
    await b.cancelScan(id)
    const summary = await done
    expect(summary.cancelled).toBe(true)
    expect(summary.entries).toBeLessThan(20000)
    expect(summary.scanId).toBe(id)
  })
})
