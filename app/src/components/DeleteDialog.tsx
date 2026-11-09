import { useEffect, useState } from 'react'
import { useScanStore } from '../store/scanStore'
import { basename, confirmValid } from '../lib/delete'
import { formatBytes } from '../lib/format'
import { SAFETY_DOT, SAFETY_LABEL, riskyItems } from '../lib/safety'
import { CheckIcon, CopyIcon, TrashIcon, XIcon } from './icons'
import { Button, Modal } from './ui'

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

export function DeleteDialog() {
  const dialog = useScanStore((s) => s.dialog)
  const close = useScanStore((s) => s.closeDialog)
  const runDelete = useScanStore((s) => s.runDelete)
  const retryFailed = useScanStore((s) => s.retryFailed)
  const notify = useScanStore((s) => s.notify)
  const [text, setText] = useState('')

  useEffect(() => {
    setText('')
  }, [dialog?.mode, dialog?.items])

  if (!dialog) return null

  const { mode, items, results, busy } = dialog
  const total = items.reduce((acc, i) => acc + i.size, 0)
  const permanent = mode === 'permanent'
  const valid = confirmValid(mode, items, text)
  const multiPermanent = permanent && items.length !== 1
  const okSet = new Set(results?.ok ?? [])
  const failMap = new Map((results?.failed ?? []).map((f) => [f.path, f.error]))
  const failedCount = results?.failed.length ?? 0
  const risky = results ? [] : riskyItems(items)
  const blocked = risky.filter((i) => i.safety === 'keep')
  const commands = results ? [] : items.filter((i) => i.safetyCommand)
  const needsRoot = commands.some((i) => i.safetyNeedsRoot)

  const copy = async (text: string) => {
    try {
      if (!navigator.clipboard) throw new Error('剪贴板不可用')
      await navigator.clipboard.writeText(text)
      notify('success', '已复制建议命令')
    } catch (e) {
      notify('error', errMsg(e))
    }
  }

  return (
    <Modal
      title={
        <span className="flex items-center gap-2">
          <span className={`grid h-5 w-5 place-items-center rounded ${permanent ? 'bg-danger/10 text-danger' : 'bg-warn/10 text-warn'}`}>
            <TrashIcon size={13} />
          </span>
          {permanent ? '永久删除' : '移入回收站'}
        </span>
      }
      onClose={close}
      footer={
        results ? (
          <>
            {failedCount > 0 && !multiPermanent && (
              <Button onClick={() => void retryFailed(text)} disabled={busy}>
                重试失败项 ({failedCount})
              </Button>
            )}
            <Button variant="primary" onClick={close}>
              关闭
            </Button>
          </>
        ) : (
          <>
            <Button onClick={close} disabled={busy}>
              取消
            </Button>
            <Button
              variant={permanent ? 'danger' : 'primary'}
              disabled={!valid || busy || multiPermanent}
              onClick={() => void runDelete(text)}
            >
              {busy ? '处理中…' : permanent ? '永久删除' : '移入回收站'}
            </Button>
          </>
        )
      }
    >
      <div className="mb-2 text-xs text-muted">
        共 {items.length} 项 · 合计 <b className="tabular-nums text-fg">{formatBytes(total)}</b>
        {permanent ? ' · 不可恢复' : ' · 可从回收站恢复'}
      </div>

      <div className="max-h-52 overflow-auto rounded-md border border-line">
        {items.map((item) => {
          const failed = failMap.get(item.path)
          const ok = okSet.has(item.path)
          return (
            <div key={item.id} className="flex items-start gap-2 border-b border-line/60 px-2 py-1.5 text-xs last:border-0">
              <span className="mt-0.5 w-4 shrink-0">
                {failed ? (
                  <XIcon size={12} className="text-danger" />
                ) : ok ? (
                  <CheckIcon size={12} className="text-safe" />
                ) : (
                  <span className="block h-3 w-3 rounded-full bg-line" />
                )}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate font-medium text-fg/90">{item.name}</span>
                <span className="block truncate font-mono text-[10px] text-muted" title={item.path}>
                  {item.path}
                </span>
                {failed && <span className="mt-0.5 block text-[10px] text-danger">{failed}</span>}
              </span>
              <span className="shrink-0 tabular-nums text-muted">{formatBytes(item.size)}</span>
            </div>
          )
        })}
      </div>

      {risky.length > 0 && (
        <div className="mt-3 rounded-md border border-warn/30 bg-warn/5 px-2.5 py-2 text-xs">
          <div className="text-warn">
            ⚠ {risky.length} 项不是安全级，删除前请确认
            {blocked.length > 0 && ` · 其中 ${blocked.length} 项后端会直接拒绝`}
          </div>
          <ul className="mt-1 max-h-24 space-y-0.5 overflow-auto">
            {risky.map((i) => (
              <li key={i.id} className="flex items-start gap-1.5 text-[11px] text-muted">
                <span className={`mt-1 h-1.5 w-1.5 shrink-0 rounded-full ${SAFETY_DOT[i.safety ?? 'unknown']}`} />
                <span className="shrink-0 text-fg/80">{i.name}</span>
                <span className="min-w-0 truncate">{i.safetyReason ?? SAFETY_LABEL[i.safety ?? 'unknown']}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {commands.length > 0 && (
        <div className="mt-3 rounded-md border border-line bg-panel2 px-2.5 py-2 text-[11px] text-muted">
          <div className="mb-1 font-medium text-fg/80">建议命令（复制后自行在终端执行）</div>
          {commands.map((i) => (
            <div key={i.id} className="mb-1 flex items-center gap-2 last:mb-0">
              <code
                className="min-w-0 flex-1 truncate rounded bg-bg px-1.5 py-0.5 font-mono text-[10px] text-fg/80"
                title={i.safetyCommand}
              >
                {i.safetyCommand}
              </code>
              <button
                type="button"
                onClick={() => void copy(i.safetyCommand ?? '')}
                className="flex shrink-0 items-center gap-1 text-accent hover:underline"
              >
                <CopyIcon size={11} />
                复制
              </button>
            </div>
          ))}
          {needsRoot && <div className="mt-1 text-[10px] text-warn">含 sudo，需要管理员权限</div>}
        </div>
      )}

      {!results && (
        <div className="mt-3">
          {multiPermanent ? (
            <div className="rounded-md border border-warn/30 bg-warn/5 px-2.5 py-2 text-xs text-warn">
              永久删除一次只支持一项（需要输入目标名称确认）。多项请使用「移入回收站」。
            </div>
          ) : permanent ? (
            <>
              <label htmlFor="confirm-name" className="mb-1 block text-xs text-muted">
                输入 <code className="rounded bg-panel2 px-1 font-mono text-[11px] text-fg">{basename(items[0].path)}</code>{' '}
                以确认永久删除
              </label>
              <input
                id="confirm-name"
                autoFocus
                value={text}
                spellCheck={false}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && valid) void runDelete(text)
                }}
                className="h-8 w-full rounded-md border border-line bg-panel2 px-2 font-mono text-xs text-fg outline-none focus:border-danger"
                placeholder={basename(items[0].path)}
              />
              {text.length > 0 && !valid && <div className="mt-1 text-[10px] text-danger">名称不匹配</div>}
            </>
          ) : (
            <div className="text-xs text-muted">
              将移动到系统回收站，可在文件管理器中恢复。受保护的系统路径会被后端拒绝。
            </div>
          )}
        </div>
      )}

      {results && (
        <div className="mt-2 text-[11px] text-muted">
          成功 {results.ok.length} 项 · 失败 {results.failed.length} 项
          {failedCount > 0 && ' · 失败项保留在上方，可重试'}
        </div>
      )}
    </Modal>
  )
}
