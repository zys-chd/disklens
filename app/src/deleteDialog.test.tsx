import { afterEach, describe, expect, it } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { DeleteDialog } from './components/DeleteDialog'
import { useScanStore } from './store/scanStore'

const item = (id: number, name: string) => ({
  id,
  name,
  size: 1024,
  path: `/home/zys/Downloads/${name}`,
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
