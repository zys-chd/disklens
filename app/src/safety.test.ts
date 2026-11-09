import { describe, expect, it } from 'vitest'
import { SAFETY_DOT, safetyDot, safetySummary, safetyTitle } from './lib/safety'
import type { SafetyReport } from './lib/types'

const report = (safeBytes: number, carefulBytes: number): SafetyReport => ({
  stats: [],
  safeBytes,
  carefulBytes,
})

describe('safety dot mapping', () => {
  it('maps every level to its color class', () => {
    expect(safetyDot({ safety: 'safe' })).toBe('bg-safe')
    expect(safetyDot({ safety: 'careful' })).toBe('bg-warn')
    expect(safetyDot({ safety: 'keep' })).toBe('bg-danger')
    expect(safetyDot({ safety: 'unknown' })).toBe('bg-muted/40')
    expect(SAFETY_DOT).toHaveProperty('safe')
  })

  it('treats a missing level as unknown (gray)', () => {
    expect(safetyDot({})).toBe('bg-muted/40')
  })
})

describe('safety summary line', () => {
  it('formats bytes for the bar', () => {
    expect(safetySummary(report(180 * 1024 * 1024, 11 * 1024 ** 3))).toEqual({
      safe: '180.0 MB',
      careful: '11.0 GB',
    })
  })

  it('shows 0 B for a real zero, — when there is no report', () => {
    expect(safetySummary(report(0, 0))).toEqual({ safe: '0 B', careful: '0 B' })
    expect(safetySummary(null)).toEqual({ safe: '—', careful: '—' })
  })
})

describe('safety tooltip', () => {
  it('joins category and reason, skipping missing parts', () => {
    expect(safetyTitle({ safetyCategory: '应用缓存', safetyReason: '自动重建' })).toBe('应用缓存 · 自动重建')
    expect(safetyTitle({ safetyCategory: '应用缓存' })).toBe('应用缓存')
    expect(safetyTitle({})).toBe('')
  })
})
