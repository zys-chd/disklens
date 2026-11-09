import { useEffect, type ButtonHTMLAttributes, type ReactNode } from 'react'
import { XIcon } from './icons'

type Variant = 'default' | 'primary' | 'danger' | 'ghost'

const VARIANTS: Record<Variant, string> = {
  default:
    'border border-line bg-panel2 text-fg hover:bg-hover disabled:opacity-45',
  primary:
    'bg-accent text-white hover:bg-accent/90 disabled:opacity-45 shadow-sm',
  danger:
    'bg-danger text-white hover:bg-danger/90 disabled:opacity-45 shadow-sm',
  ghost: 'text-muted hover:text-fg hover:bg-hover disabled:opacity-40',
}

export function Button({
  variant = 'default',
  className = '',
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      type="button"
      className={`inline-flex h-7 items-center justify-center gap-1.5 rounded-md px-2.5 text-xs font-medium transition-colors disabled:cursor-not-allowed ${VARIANTS[variant]} ${className}`}
      {...props}
    />
  )
}

export function Modal({
  title,
  onClose,
  children,
  footer,
  width = 520,
}: {
  title: ReactNode
  onClose: () => void
  children: ReactNode
  footer?: ReactNode
  width?: number
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6 backdrop-blur-[1px]"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        className="animate-in flex max-h-[80vh] w-full flex-col rounded-xl border border-line bg-panel shadow-2xl"
        style={{ maxWidth: width }}
      >
        <div className="flex h-11 shrink-0 items-center justify-between border-b border-line px-4">
          <div className="text-[13px] font-semibold">{title}</div>
          <button
            type="button"
            aria-label="关闭"
            className="rounded-md p-1 text-muted hover:bg-hover hover:text-fg"
            onClick={onClose}
          >
            <XIcon size={15} />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-4 py-3">{children}</div>
        {footer && (
          <div className="flex shrink-0 items-center justify-end gap-2 border-t border-line px-4 py-2.5">
            {footer}
          </div>
        )}
      </div>
    </div>
  )
}

export function Spinner({ size = 16 }: { size?: number }) {
  return (
    <svg
      className="spin text-accent"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-label="加载中"
    >
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeOpacity="0.2" strokeWidth="3" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  )
}

export function Empty({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1.5 p-6 text-center">
      <div className="text-[13px] font-medium text-fg/80">{title}</div>
      {hint && <div className="max-w-[280px] text-xs text-muted">{hint}</div>}
    </div>
  )
}
