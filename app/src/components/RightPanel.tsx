import { useState } from 'react'
import { TreemapCanvas } from './TreemapCanvas'
import { Sunburst } from './Sunburst'
import { TypeStats } from './TypeStats'

const TABS = [
  { key: 'treemap', label: 'Treemap' },
  { key: 'sunburst', label: '旭日图' },
  { key: 'types', label: '类型分布' },
] as const

type Tab = (typeof TABS)[number]['key']

export function RightPanel() {
  const [tab, setTab] = useState<Tab>('treemap')

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-9 shrink-0 items-center gap-1 border-b border-line px-2">
        {TABS.map((t) => (
          <button
            key={t.key}
            type="button"
            onClick={() => setTab(t.key)}
            className={`rounded-md px-2 py-1 text-[11px] font-medium transition-colors ${
              tab === t.key ? 'bg-sel text-accent' : 'text-muted hover:bg-hover hover:text-fg'
            }`}
          >
            {t.label}
          </button>
        ))}
        <div className="flex-1" />
        <span className="pr-1 text-[10px] text-muted">点击下钻</span>
      </div>
      <div className="min-h-0 flex-1">
        {tab === 'treemap' && <TreemapCanvas />}
        {tab === 'sunburst' && <Sunburst />}
        {tab === 'types' && <TypeStats />}
      </div>
    </div>
  )
}
