import { describe, expect, it } from 'vitest'
import { formatBytes, formatCount, percent } from './lib/format'

describe('formatBytes', () => {
  it('handles zero and sub-KB as integer bytes', () => {
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(-1)).toBe('0 B')
    expect(formatBytes(1)).toBe('1 B')
    expect(formatBytes(1023)).toBe('1023 B')
  })

  it('uses 1024 base with one decimal', () => {
    expect(formatBytes(1024)).toBe('1.0 KB')
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatBytes(1024 * 1024)).toBe('1.0 MB')
    expect(formatBytes(1610612736)).toBe('1.5 GB')
    expect(formatBytes(1099511627776)).toBe('1.0 TB')
    expect(formatBytes(1649267441664)).toBe('1.5 TB')
  })

  it('carries over when rounding would print 1024.0 of a unit', () => {
    expect(formatBytes(1048575)).toBe('1.0 MB')
    expect(formatBytes(1073741823)).toBe('1.0 GB')
  })
})

describe('formatCount / percent', () => {
  it('groups thousands with commas', () => {
    expect(formatCount(0)).toBe('0')
    expect(formatCount(42183)).toBe('42,183')
    expect(formatCount(1000000)).toBe('1,000,000')
  })

  it('never divides by zero', () => {
    expect(percent(0, 0)).toBe(0)
    expect(percent(25, 100)).toBe(25)
  })
})
