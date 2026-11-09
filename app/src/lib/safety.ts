import { formatBytes } from './format'
import type { NodeView, SafetyLevel, SafetyReport } from './types'

/** 圆点颜色：safe 绿 / careful 黄 / keep 红 / 未分类灰。 */
export const SAFETY_DOT: Record<SafetyLevel, string> = {
  safe: 'bg-safe',
  careful: 'bg-warn',
  keep: 'bg-danger',
  unknown: 'bg-muted/40',
}

export const SAFETY_LABEL: Record<SafetyLevel, string> = {
  safe: '🟢 可安全清理',
  careful: '🟡 谨慎重建',
  keep: '🔴 不要动',
  unknown: '❓ 待人工判断',
}

export function safetyDot(node: Pick<NodeView, 'safety'>): string {
  return SAFETY_DOT[node.safety ?? 'unknown']
}

/** 树行 hover 提示：category + reason。 */
export function safetyTitle(node: Pick<NodeView, 'safetyCategory' | 'safetyReason'>): string {
  return [node.safetyCategory, node.safetyReason].filter(Boolean).join(' · ')
}

/** 汇总条文案：无报告用 —（不是 0 B）；真的有 0 字节就照实显示 0 B。 */
export function safetySummary(report: SafetyReport | null): { safe: string; careful: string } {
  return report
    ? { safe: formatBytes(report.safeBytes), careful: formatBytes(report.carefulBytes) }
    : { safe: '—', careful: '—' }
}

/** 删除前需要警示的项：careful 可能被删，keep 后端会直接拒。 */
export function riskyItems<T extends { safety?: SafetyLevel }>(items: T[]): T[] {
  return items.filter((i) => i.safety === 'careful' || i.safety === 'keep')
}
