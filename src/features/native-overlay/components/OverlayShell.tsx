import type { OverlayShellProps } from '../types'
import { CompactIsland } from './CompactIsland'
import styles from './overlay.module.css'
import { PeekPanel } from './PeekPanel'

export function OverlayShell({
  mode,
  renderContext,
  platform,
  reducedMotion,
  connectionState,
  snapshot,
  cpuHistory = [],
  staleMessage,
  errorMessage,
  nowMs = Date.now(),
  onOpenDashboard,
  onAcknowledge,
  pendingDecision,
  decisionControlsEnabled,
  onDecisionAllow,
  onDecisionDeny,
  emptyMessage,
}: OverlayShellProps) {
  return (
    <div
      className={styles.overlayRoot}
      data-mode={mode}
      data-render-context={renderContext}
      data-platform={platform}
      data-reduced-motion={reducedMotion ? 'true' : 'false'}
      data-testid="overlay-shell"
    >
      {renderContext === 'preview' ? (
        <span className={styles.previewBadge} data-testid="preview-badge">
          Preview
        </span>
      ) : null}

      {mode === 'compact' ? (
        <CompactIsland
          platform={platform}
          connectionState={connectionState}
          snapshot={snapshot}
          cpuHistory={cpuHistory}
          nowMs={nowMs}
          reducedMotion={reducedMotion}
          emptyMessage={emptyMessage}
          staleMessage={staleMessage}
          errorMessage={errorMessage}
        />
      ) : (
        <PeekPanel
          platform={platform}
          connectionState={connectionState}
          snapshot={snapshot}
          staleMessage={staleMessage}
          errorMessage={errorMessage}
          emptyMessage={emptyMessage}
          onOpenDashboard={onOpenDashboard}
          onAcknowledge={onAcknowledge}
          pendingDecision={pendingDecision}
          decisionControlsEnabled={decisionControlsEnabled}
          onDecisionAllow={onDecisionAllow}
          onDecisionDeny={onDecisionDeny}
        />
      )}
    </div>
  )
}
