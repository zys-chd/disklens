export type DeleteMode = 'trash' | 'permanent'

export interface DeleteItem {
  id: number
  name: string
  size: number
  path: string
}

export function basename(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean)
  return parts.length ? parts[parts.length - 1] : path
}

/**
 * 永久删除要求输入目标 basename（Rust guard 的硬约束）。
 * 多选时一个输入框无法对应多个 basename，所以只允许单项永久删除。
 */
export function confirmValid(mode: DeleteMode, items: DeleteItem[], text: string): boolean {
  if (mode === 'trash') return true
  if (items.length !== 1) return false
  return text.trim() === basename(items[0].path)
}
