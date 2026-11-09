import { useScanStore } from '../store/scanStore'
import { backend } from '../lib/backend'
import { safetySummary } from '../lib/safety'

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

/** 清理建议汇总条：只看可清理开关 + 规则文件入口。 */
export function SafetyBar() {
  const status = useScanStore((s) => s.status)
  const report = useScanStore((s) => s.safety)
  const onlySafe = useScanStore((s) => s.onlySafe)
  const setOnlySafe = useScanStore((s) => s.setOnlySafe)
  const notify = useScanStore((s) => s.notify)

  if (status !== 'done' || !report) return null
  const { safe, careful } = safetySummary(report)

  return (
    <div className="flex h-8 shrink-0 items-center gap-3 border-b border-line bg-panel px-3 text-[11px]">
      <span className="flex items-center gap-1.5">
        <span className="h-2 w-2 rounded-full bg-safe" aria-hidden />
        可安全清理 <b className="tabular-nums text-fg">{safe}</b>
      </span>
      <span className="flex items-center gap-1.5">
        <span className="h-2 w-2 rounded-full bg-warn" aria-hidden />
        谨慎重建 <b className="tabular-nums text-fg">{careful}</b>
      </span>
      <div className="flex-1" />
      <button
        type="button"
        aria-pressed={onlySafe}
        onClick={() => void setOnlySafe(!onlySafe)}
        className={`rounded-md border px-2 py-0.5 text-[11px] ${
          onlySafe ? 'border-safe/40 bg-safe/10 font-medium text-safe' : 'border-line text-muted hover:text-fg'
        }`}
      >
        只看可清理
      </button>
      <button
        type="button"
        onClick={() => void backend.openRulesFile().catch((e) => notify('error', errMsg(e)))}
        className="rounded-md px-1.5 py-0.5 text-[11px] text-muted hover:bg-hover hover:text-fg"
        title="编辑 rules.json（分级规则）"
      >
        规则
      </button>
    </div>
  )
}
