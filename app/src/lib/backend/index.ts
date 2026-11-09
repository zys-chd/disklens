import type { Backend } from '../types'
import { mockBackend } from './mock'
import { tauriBackend } from './tauri'

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export const backend: Backend = isTauri() ? tauriBackend : mockBackend
