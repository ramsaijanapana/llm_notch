import { describe, expect, it, vi } from 'vitest'
import * as environment from '../native/environment.ts'
import * as previewRouting from '../native/previewRouting.ts'
import { resolveDesktopSurface } from './ViewRouter.tsx'

describe('resolveDesktopSurface', () => {
  it('maps overlay and dashboard labels from Tauri', async () => {
    const labelSpy = vi.spyOn(environment, 'getTauriWindowLabel')
    const previewSpy = vi.spyOn(previewRouting, 'resolveNativePreviewSurface')

    previewSpy.mockReturnValue(null)

    labelSpy.mockResolvedValueOnce('overlay')
    await expect(resolveDesktopSurface()).resolves.toBe('overlay')

    labelSpy.mockResolvedValueOnce('dashboard')
    await expect(resolveDesktopSurface()).resolves.toBe('dashboard')

    labelSpy.mockResolvedValueOnce(null)
    await expect(resolveDesktopSurface()).resolves.toBe('preview')

    labelSpy.mockResolvedValueOnce('settings')
    await expect(resolveDesktopSurface()).resolves.toBe('unknown')
  })

  it('prefers test-only nativePreview routing in browser mode', async () => {
    vi.spyOn(previewRouting, 'resolveNativePreviewSurface').mockReturnValue('overlay')
    await expect(resolveDesktopSurface()).resolves.toBe('overlay')
  })
})
