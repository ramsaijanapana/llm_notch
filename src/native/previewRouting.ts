import { isTauriEnvironment } from './environment.ts'

export type NativePreviewSurface = 'overlay' | 'dashboard'

export type NativePreviewScenario = 'disconnected' | 'incompatible' | 'resync' | 'loading' | null

function readSearchParam(name: string): string | null {
  if (typeof window === 'undefined') {
    return null
  }

  return new URLSearchParams(window.location.search).get(name)
}

/** True when the browser bundle should mount DesktopApp via test-only preview routing. */
export function isNativePreviewActive(): boolean {
  if (isTauriEnvironment()) {
    return false
  }

  return resolveNativePreviewSurface() !== null
}

/** Test-only surface override from `?nativePreview=overlay|dashboard`. */
export function resolveNativePreviewSurface(): NativePreviewSurface | null {
  if (isTauriEnvironment()) {
    return null
  }

  const value = readSearchParam('nativePreview')?.trim().toLowerCase()

  if (value === 'overlay' || value === 'dashboard') {
    return value
  }

  const envSurface = import.meta.env.VITE_NATIVE_PREVIEW?.trim().toLowerCase()
  if (envSurface === 'overlay' || envSurface === 'dashboard') {
    return envSurface
  }

  return null
}

/** Optional preview scenario from `?nativeScenario=...` for deterministic E2E states. */
export function resolveNativePreviewScenario(): NativePreviewScenario {
  if (!isNativePreviewActive()) {
    return null
  }

  const value = readSearchParam('nativeScenario')?.trim().toLowerCase()

  switch (value) {
    case 'disconnected':
    case 'incompatible':
    case 'resync':
    case 'loading':
      return value
    default:
      return null
  }
}
