import { type ComponentType, type LazyExoticComponent, lazy, type ReactNode, Suspense } from 'react'
import { getTauriWindowLabel } from '../native/environment.ts'
import { resolveNativePreviewSurface } from '../native/previewRouting.ts'
import styles from './ViewRouter.module.css'

export type DesktopSurface = 'overlay' | 'dashboard' | 'preview' | 'unknown'

export interface ViewRouterProps {
  surface?: DesktopSurface
  prefersReducedMotion?: boolean
  overlay?: ComponentType | LazyExoticComponent<ComponentType> | ReactNode
  dashboard?: ComponentType | LazyExoticComponent<ComponentType> | ReactNode
  fallbackOverlay?: ReactNode
  fallbackDashboard?: ReactNode
}

function PlaceholderPanel({
  title,
  copy,
  loading = false,
}: {
  title: string
  copy: string
  loading?: boolean
}) {
  return (
    <section className={styles.placeholder} aria-busy={loading}>
      <h1 className={styles.placeholderTitle}>{title}</h1>
      <p className={styles.placeholderCopy}>{copy}</p>
      {loading ? (
        <div className={styles.loading} role="status" aria-live="polite">
          <span className={styles.spinner} aria-hidden="true" />
          <span>Connecting to native host…</span>
        </div>
      ) : null}
    </section>
  )
}

function isRenderableComponent(
  slot: ViewRouterProps['overlay'],
): slot is ComponentType | LazyExoticComponent<ComponentType> {
  return (
    typeof slot === 'function' || (typeof slot === 'object' && slot !== null && '$$typeof' in slot)
  )
}

function renderSlot(slot: ViewRouterProps['overlay'], fallback: ReactNode): ReactNode {
  if (!slot) {
    return fallback
  }

  if (isRenderableComponent(slot)) {
    const Component = slot
    return (
      <Suspense fallback={fallback}>
        <Component />
      </Suspense>
    )
  }

  return slot
}

export async function resolveDesktopSurface(): Promise<DesktopSurface> {
  const previewSurface = resolveNativePreviewSurface()
  if (previewSurface) {
    return previewSurface
  }

  const label = await getTauriWindowLabel()

  if (label === 'overlay') {
    return 'overlay'
  }

  if (label === 'dashboard') {
    return 'dashboard'
  }

  if (label === null) {
    return 'preview'
  }

  return 'unknown'
}

export function ViewRouter({
  surface = 'preview',
  prefersReducedMotion = false,
  overlay,
  dashboard,
  fallbackOverlay,
  fallbackDashboard,
}: ViewRouterProps) {
  const overlayFallback = fallbackOverlay ?? (
    <PlaceholderPanel
      title="Overlay surface"
      copy="Overlay UI will mount here once the overlay feature module is wired."
      loading={surface === 'overlay'}
    />
  )

  const dashboardFallback = fallbackDashboard ?? (
    <PlaceholderPanel
      title="Dashboard surface"
      copy="Dashboard UI will mount here once the dashboard feature module is wired."
      loading={surface === 'dashboard'}
    />
  )

  let content: ReactNode

  switch (surface) {
    case 'overlay':
      content = renderSlot(overlay, overlayFallback)
      break
    case 'dashboard':
      content = renderSlot(dashboard, dashboardFallback)
      break
    case 'preview':
      content = (
        <>
          {renderSlot(overlay, overlayFallback)}
          {renderSlot(dashboard, dashboardFallback)}
        </>
      )
      break
    default:
      content = (
        <PlaceholderPanel
          title="Unsupported window"
          copy="This window label is not mapped to overlay or dashboard chrome."
        />
      )
      break
  }

  return (
    <div
      className={`${styles.viewport} ${prefersReducedMotion ? styles.reducedMotion : ''}`}
      data-surface={surface}
      data-reduced-motion={prefersReducedMotion ? 'true' : 'false'}
    >
      {content}
    </div>
  )
}

/** Lazy helper for feature modules that are not yet available at compile time. */
export function createLazyView(loader: () => Promise<{ default: ComponentType }>) {
  return lazy(loader)
}
