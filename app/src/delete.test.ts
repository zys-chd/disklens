import { describe, expect, it } from 'vitest'
import { basename, confirmValid, type DeleteItem } from './lib/delete'

const item = (id: number, path: string, name = basename(path)): DeleteItem => ({
  id,
  name,
  path,
  size: 1024,
})

describe('basename', () => {
  it('handles posix, windows and trailing slashes', () => {
    expect(basename('/home/zys/Downloads/large.bin')).toBe('large.bin')
    expect(basename('C:\\Users\\zys\\file.txt')).toBe('file.txt')
    expect(basename('/home/zys/Projects/')).toBe('Projects')
    expect(basename('plain')).toBe('plain')
  })
})

describe('confirmValid', () => {
  it('trash needs no typed confirmation', () => {
    expect(confirmValid('trash', [item(1, '/home/zys/.cache/a'), item(2, '/home/zys/.cache/b')], '')).toBe(true)
  })

  it('permanent delete requires the exact basename', () => {
    const items = [item(1, '/home/zys/Downloads/large.bin')]
    expect(confirmValid('permanent', items, '')).toBe(false)
    expect(confirmValid('permanent', items, 'large')).toBe(false)
    expect(confirmValid('permanent', items, 'large.bin')).toBe(true)
    expect(confirmValid('permanent', items, '  large.bin  ')).toBe(true)
    expect(confirmValid('permanent', items, 'LARGE.BIN')).toBe(false)
  })

  it('refuses permanent delete of multiple items', () => {
    const items = [item(1, '/home/zys/.cache/a'), item(2, '/home/zys/.cache/b')]
    expect(confirmValid('permanent', items, 'a')).toBe(false)
    expect(confirmValid('permanent', items, 'b')).toBe(false)
  })
})
