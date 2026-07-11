import {
  type ComponentType,
  type LazyExoticComponent,
  type ReactNode,
  useEffect,
  useState,
} from 'react'
import { createNativeClient } from '../native/client.ts'
import { clearPreviewBridge, installPreviewBridge } from '../native/previewBridge.ts'
import { resolveNativePreviewSurface } from '../native/previewRouting.ts'
import type { NativeClient } from '../native/types.ts'
import { NativeStateProvider, useNativeState } from '../state/NativeStateProvider.tsx'
import styles from './DesktopApp.module.css'
import { NativeDashboardSurface, NativeOverlaySurface } from './NativeSurfaces.tsx'
import { type DesktopSurface, resolveDesktopSurface, ViewRouter } from './ViewRouter.tsx'

export interface DesktopAppProps {
  client?: NativeClient
  overlay?: ComponentType | LazyExoticComponent<ComponentType> | ReactNode
  dashboard?: ComponentType | LazyExoticComponent<ComponentType> | ReactNode
  fallbackOverlay?: ReactNode
  fallbackDashboard?: ReactNode
}

function ConnectionBanners({ surface }: { surface: DesktopSurface }) {
  const { state } = useNativeState()

  if (surface === 'overlay') {
    return null
  }

  const banners: Array<{ id: string; tone: 'info' | 'warning' | 'error'; message: string }> = []

  if (state.clientMode === 'preview') {
    banners.push({
      id: 'preview',
      tone: 'warning',
      message:
        'Preview / test mode — this renderer is using a simulated native host, not production desktop telemetry.',
    })
  }

  if (state.connection === 'loading') {
    banners.push({
      id: 'loading',
      tone: 'info',
      message: 'Loading native snapshot and live stream…',
    })
  }

  if (state.connection === 'incompatible-protocol') {
    banners.push({
      id: 'protocol',
      tone: 'error',
      message:
        state.errorMessage ?? 'The desktop host protocol is incompatible with this renderer build.',
    })
  }

  if (state.connection === 'disconnected') {
    banners.push({
      id: 'disconnected',
      tone: 'error',
      message: state.errorMessage ?? 'Disconnected from the native host.',
    })
  }

  if (state.connection === 'resyncing') {
    banners.push({
      id: 'resync',
      tone: 'warning',
      message: state.resyncReason
        ? `Resyncing native stream: ${state.resyncReason}`
        : 'Resyncing native stream after sequence gap.',
    })
  }

  return (
    <div className={styles.statusRegion} aria-live="polite" aria-atomic="false">
      {banners.map((banner) => (
        <div
          key={banner.id}
          className={banner.id === 'preview' ? styles.previewBanner : styles.stateBanner}
          data-tone={banner.tone}
          role={banner.tone === 'error' ? 'alert' : 'status'}
        >
          {banner.message}
        </div>
      ))}
    </div>
  )
}

function DesktopAppBody({
  surface,
  overlay,
  dashboard,
  fallbackOverlay,
  fallbackDashboard,
}: {
  surface: DesktopSurface
  overlay?: DesktopAppProps['overlay']
  dashboard?: DesktopAppProps['dashboard']
  fallbackOverlay?: ReactNode
  fallbackDashboard?: ReactNode
}) {
  const { prefersReducedMotion } = useNativeState()

  return (
    <div
      className={`${styles.shell} ${prefersReducedMotion ? styles.reducedMotion : ''}`}
      data-surface={surface}
      data-reduced-motion={prefersReducedMotion ? 'true' : 'false'}
    >
      <ConnectionBanners surface={surface} />
      <main className={styles.content}>
        <ViewRouter
          surface={surface}
          prefersReducedMotion={prefersReducedMotion}
          overlay={overlay}
          dashboard={dashboard}
          fallbackOverlay={fallbackOverlay}
          fallbackDashboard={fallbackDashboard}
        />
      </main>
    </div>
  )
}

export function DesktopApp({
  client,
  overlay,
  dashboard,
  fallbackOverlay,
  fallbackDashboard,
}: DesktopAppProps) {
  const [resolvedClient] = useState(() => client ?? createNativeClient())
  const [surface, setSurface] = useState<DesktopSurface>(
    () =>
      resolveNativePreviewSurface() ?? (resolvedClient.mode === 'preview' ? 'preview' : 'unknown'),
  )

  useEffect(() => {
    let cancelled = false

    void resolveDesktopSurface().then((nextSurface) => {
      if (!cancelled) {
        setSurface(nextSurface)
      }
    })

    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    document.documentElement.dataset.nativeSurface = surface
    return () => {
      delete document.documentElement.dataset.nativeSurface
    }
  }, [surface])

  useEffect(() => {
    installPreviewBridge(resolvedClient)
    return () => {
      clearPreviewBridge()
    }
  }, [resolvedClient])

  return (
    <NativeStateProvider client={resolvedClient}>
      <DesktopAppBody
        surface={surface}
        overlay={overlay ?? NativeOverlaySurface}
        dashboard={dashboard ?? NativeDashboardSurface}
        fallbackOverlay={fallbackOverlay}
        fallbackDashboard={fallbackDashboard}
      />
    </NativeStateProvider>
  )
}
