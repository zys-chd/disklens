import { afterEach, describe, expect, it } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { DeleteDialog } from './components/DeleteDialog'
import { useScanStore } from './store/scanStore'
import type { DeleteItem } from './lib/delete'

const base = (id: number, name: string): DeleteItem => ({
  id,
  name,
  size: 1024,
  path: `/home/zys/Downloads/${name}`,
})

const item = (id: number, name: string, extra: Partial<DeleteItem> = {}): DeleteItem => ({
  ...base(id, name),
  ...extra,
})

afterEach(() => {
  useScanStore.setState({ dialog: null })
})

describe('DeleteDialog confirmText gate', () => {
  it('permanent delete stays disabled until the exact basename is typed', () => {
    useScanStore.setState({
      dialog: { mode: 'permanent', items: [item(1, 'large.bin')], results: null, busy: false },
    })
    render(<DeleteDialog />)

    const confirm = screen.getByRole('button', { name: '永久删除' }) as HTMLButtonElement
    const input = screen.getByPlaceholderText('large.bin')
    expect(confirm.disabled).toBe(true)

    fireEvent.change(input, { target: { value: 'large' } })
    expect(confirm.disabled).toBe(true)
    expect(screen.getByText('名称不匹配')).toBeTruthy()

    fireEvent.change(input, { target: { value: 'large.bin' } })
    expect(confirm.disabled).toBe(false)
  })

  it('trash confirmation is immediately available for multiple items', () => {
    useScanStore.setState({
      dialog: {
        mode: 'trash',
        items: [item(1, 'a.cache'), item(2, 'b.cache')],
        results: null,
        busy: false,
      },
    })
    render(<DeleteDialog />)
    const confirm = screen.getByRole('button', { name: '移入回收站' }) as HTMLButtonElement
    expect(confirm.disabled).toBe(false)
    expect(screen.getByText(/共 2 项/)).toBeTruthy()
  })

  it('refuses permanent multi-delete and explains why', () => {
    useScanStore.setState({
      dialog: {
        mode: 'permanent',
        items: [item(1, 'a.cache'), item(2, 'b.cache')],
        results: null,
        busy: false,
      },
    })
    render(<DeleteDialog />)
    const confirm = screen.getByRole('button', { name: '永久删除' }) as HTMLButtonElement
    expect(confirm.disabled).toBe(true)
    expect(screen.getByText(/永久删除一次只支持一项/)).toBeTruthy()
  })
})

describe('DeleteDialog safety warnings', () => {
  it('warns about careful/keep items and names the blocked ones', () => {
    useScanStore.setState({
      dialog: {
        mode: 'trash',
        items: [
          item(1, '.ssh', { safety: 'keep', safetyReason: 'SSH 密钥' }),
          item(2, 'node_modules', { safety: 'careful', safetyReason: '删除后需重新 npm install' }),
          item(3, 'pip-cache', { safety: 'safe' }),
        ],
        results: null,
        busy: false,
      },
    })
    render(<DeleteDialog />)
    expect(screen.getByText(/2 项不是安全级/)).toBeTruthy()
    expect(screen.getByText(/1 项后端会直接拒绝/)).toBeTruthy()
    expect(screen.getByText('SSH 密钥')).toBeTruthy()
    expect(screen.getByText('删除后需重新 npm install')).toBeTruthy()
  })

  it('stays quiet when everything is safe', () => {
    useScanStore.setState({
      dialog: { mode: 'trash', items: [item(1, 'pip-cache', { safety: 'safe' })], results: null, busy: false },
    })
    render(<DeleteDialog />)
    expect(screen.queryByText(/不是安全级/)).toBeNull()
  })

  it('shows the suggested command without executing it', () => {
    useScanStore.setState({
      dialog: {
        mode: 'trash',
        items: [
          item(1, 'journal', {
            safety: 'careful',
            safetyCommand: 'sudo journalctl --vacuum-size=100M',
            safetyNeedsRoot: true,
          }),
        ],
        results: null,
        busy: false,
      },
    })
    render(<DeleteDialog />)
    expect(screen.getByText('sudo journalctl --vacuum-size=100M')).toBeTruthy()
    expect(screen.getByText(/需要管理员权限/)).toBeTruthy()
  })
})
