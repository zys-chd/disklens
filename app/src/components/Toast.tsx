import { useScanStore } from '../store/scanStore'

const COLORS = {
  info: 'border-line text-fg',
  success: 'border-safe/40 text-safe',
  warn: 'border-warn/40 text-warn',
  error: 'border-danger/40 text-danger',
} as const

export function Toast() {
  const toast = useScanStore((s) => s.toast)
  if (!toast) return null

  return (
    <div
      role="status"
      className={`animate-in fixed bottom-4 left-1/2 z-[60] -translate-x-1/2 rounded-lg border bg-panel/95 px-3 py-1.5 text-xs shadow-xl backdrop-blur ${COLORS[toast.kind]}`}
    >
      {toast.text}
    </div>
  )
}
