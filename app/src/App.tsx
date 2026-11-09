import { useEffect } from 'react'
import { useScanStore } from './store/scanStore'
import { TopBar } from './components/TopBar'
import { Breadcrumb } from './components/Breadcrumb'
import { TreeView } from './components/TreeView'
import { RightPanel } from './components/RightPanel'
import { BottomPanel } from './components/BottomPanel'
import { SelectionBar } from './components/SelectionBar'
import { ContextMenu } from './components/ContextMenu'
import { DeleteDialog } from './components/DeleteDialog'
import { Toast } from './components/Toast'

export default function App() {
  const init = useScanStore((s) => s.init)

  useEffect(() => {
    void init()
  }, [init])

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-bg text-fg">
      <TopBar />
      <Breadcrumb />
      <div className="flex min-h-0 flex-1">
        <div className="flex min-w-0 flex-1 flex-col">
          <TreeView />
        </div>
        <aside className="flex w-[42%] min-w-[340px] max-w-[760px] shrink-0 flex-col border-l border-line bg-panel">
          <RightPanel />
        </aside>
      </div>
      <BottomPanel />
      <SelectionBar />
      <ContextMenu />
      <DeleteDialog />
      <Toast />
    </div>
  )
}
