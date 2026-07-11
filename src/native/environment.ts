/**
 * Safe runtime detection for Tauri vs browser marketing app.
 *
 * Owner: Stage 0 foundation agent. Overlay/dashboard routing agents should import
 * these helpers instead of touching `window` globals directly.
 */

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
    __TAURI__?: unknown
  }
}

/** True when the bundle is executing inside a Tauri webview. */
export function isTauriEnvironment(): boolean {
  return typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)
}

/** Best-effort current window label when running under Tauri. */
export async function getTauriWindowLabel(): Promise<string | null> {
  if (!isTauriEnvironment()) {
    return null
  }

  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    return getCurrentWindow().label
  } catch {
    return null
  }
}

/** Whether the current surface should render overlay chrome. */
export async function isOverlayWindow(): Promise<boolean> {
  const label = await getTauriWindowLabel()
  return label === 'overlay'
}

/** Whether the current surface should render dashboard chrome. */
export async function isDashboardWindow(): Promise<boolean> {
  const label = await getTauriWindowLabel()
  return label === 'dashboard'
}

/** Browser marketing build detection (inverse of Tauri). */
export function isBrowserMarketingApp(): boolean {
  return !isTauriEnvironment()
}
