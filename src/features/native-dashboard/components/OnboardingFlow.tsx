import { type KeyboardEvent, useEffect, useRef, useState } from 'react'
import {
  CheckCircle2,
  Loader2,
  Monitor,
  Radar,
  ShieldCheck,
  Sparkles,
  Zap,
} from 'lucide-react'
import styles from '../styles/dashboard.module.css'
import type { OnboardingFlowProps } from '../types/contracts'
import { agentLabel } from '../utils/formatters'
import { DOCUMENTED_CONNECTOR_PATHS } from '../utils/integrationLabels'
import { ApplyProgressPanel } from './integrations/ApplyProgressPanel'
import { DiffReviewPanel } from './integrations/DiffReviewPanel'

const STEP_TITLES = [
  'Detect agents',
  'Island & display',
  'Connect agents',
  'Review & apply',
  'Shortcuts & startup',
] as const
const AUTOMATIC_DISPLAY_VALUE = '__automatic__'

export function OnboardingFlow({
  open,
  step,
  displays,
  selectedDisplayId,
  displayLoadState = 'ready',
  displayError,
  fullscreenPreferenceSupported = true,
  onDisplayChange,
  integrationOptions,
  detectedConnectors,
  detectLoadState = 'idle',
  detectError,
  onGetStarted,
  connectSelections,
  onConnectSelectionChange,
  connectScope,
  onConnectScopeChange,
  pendingPlan,
  pendingPlanCount = 1,
  applyProgress,
  applyResult,
  onPreviewConnect,
  onConfirmApply,
  onSkipConnect,
  onTogglePlanFile,
  shortcutLabel,
  autostartEnabled,
  onAutostartChange,
  onNext,
  onBack,
  onSkip,
  onFinish,
  reducedMotion = false,
}: OnboardingFlowProps) {
  const dialogRef = useRef<HTMLDivElement>(null)
  const restoreFocusRef = useRef<HTMLElement | null>(null)
  const [confirmSkip, setConfirmSkip] = useState(false)
  const [pathsExpanded, setPathsExpanded] = useState(false)

  useEffect(() => {
    if (!open) return
    restoreFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null
    const dialog = dialogRef.current
    const focusable = dialog?.querySelector<HTMLElement>(
      'select:not([disabled]), input:not([disabled]), button:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    )
    ;(focusable ?? dialog)?.focus()

    return () => {
      restoreFocusRef.current?.focus()
      restoreFocusRef.current = null
    }
  }, [open])

  useEffect(() => {
    if (!open) {
      setConfirmSkip(false)
      setPathsExpanded(false)
    }
  }, [open])

  useEffect(() => {
    if (!open || !confirmSkip) return
    dialogRef.current?.querySelector<HTMLElement>('[role="alertdialog"] button')?.focus()
  }, [confirmSkip, open])

  if (!open) {
    return null
  }

  const overlayClass = reducedMotion
    ? `${styles.onboardingOverlay} ${styles.reduceMotion}`
    : styles.onboardingOverlay
  const cardClass = reducedMotion
    ? `${styles.onboardingCard} ${styles.onboardingCardStatic}`
    : `${styles.onboardingCard} ${styles.onboardingCardEnter}`
  const isLastStep = step === 4
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault()
      setConfirmSkip((current) => !current)
      return
    }
    if (event.key !== 'Tab') return
    const focusable = Array.from(
      event.currentTarget.querySelectorAll<HTMLElement>(
        'select:not([disabled]), input:not([disabled]), button:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    )
    if (focusable.length === 0) {
      event.preventDefault()
      event.currentTarget.focus()
      return
    }
    const first = focusable[0]
    const last = focusable.at(-1)
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last?.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first?.focus()
    }
  }

  const selectedCount = connectSelections.filter((entry) => entry.selected).length

  return (
    <div
      ref={dialogRef}
      className={overlayClass}
      role="dialog"
      aria-modal="true"
      aria-labelledby="onboarding-title"
      tabIndex={-1}
      onKeyDown={handleKeyDown}
    >
      <div className={cardClass}>
        <div className={styles.onboardingProgress} aria-hidden="true">
          {STEP_TITLES.map((title, index) => (
            <span
              key={title}
              className={
                index === step
                  ? styles.onboardingStepActive
                  : index < step
                    ? styles.onboardingStepDone
                    : styles.onboardingStep
              }
            />
          ))}
        </div>

        {step === 0 ? (
          <div className={styles.onboardingHero}>
            <span className={styles.onboardingHeroIcon} aria-hidden="true">
              <Radar size={22} strokeWidth={2} />
            </span>
            <div>
              <h2 id="onboarding-title" className={styles.onboardingHeroTitle}>
                Detect & connect your agents
              </h2>
              <p className={styles.muted}>
                One scan checks every supported agent at documented configuration paths. We never
                browse your disk — only fixed, published locations.
              </p>
            </div>
          </div>
        ) : (
          <h2 id="onboarding-title" className={styles.cardTitle}>
            {STEP_TITLES[step]}
          </h2>
        )}

        {step === 0 ? (
          <>
            <button
              type="button"
              className={styles.pathsToggle}
              aria-expanded={pathsExpanded}
              onClick={() => setPathsExpanded((current) => !current)}
            >
              <ShieldCheck size={14} strokeWidth={2} aria-hidden="true" />
              {pathsExpanded ? 'Hide documented paths' : 'How detection works'}
            </button>
            {pathsExpanded ? (
              <ul className={styles.pathList}>
                {DOCUMENTED_CONNECTOR_PATHS.map((entry) => (
                  <li key={entry.source} className={styles.pathItem}>
                    <strong>{agentLabel(entry.source)}</strong>
                    <span className={styles.mono}>{entry.userPath}</span>
                    {entry.source !== 'generic' ? (
                      <span className={styles.pathProject}>project: {entry.projectPath}</span>
                    ) : null}
                  </li>
                ))}
              </ul>
            ) : null}

            {detectLoadState === 'loading' ? (
              <p className={styles.onboardingStatus} role="status">
                <Loader2 size={14} className={styles.spinIcon} aria-hidden="true" />
                Scanning documented paths…
              </p>
            ) : null}
            {detectLoadState === 'error' ? (
              <p className={styles.caveat} role="alert">
                {detectError ?? 'Detection failed. You can retry or skip and connect later.'}
              </p>
            ) : null}
            {detectLoadState === 'ready' && detectedConnectors.length > 0 ? (
              <div className={styles.detectResults} role="status">
                <p className={styles.onboardingStatus}>
                  <CheckCircle2 size={14} aria-hidden="true" />
                  Found {detectedConnectors.length} configuration path
                  {detectedConnectors.length === 1 ? '' : 's'}
                </p>
                <ul className={styles.detectChips}>
                  {detectedConnectors.map((entry) => (
                    <li key={`${entry.source}-${entry.displayPath}`} className={styles.detectChip}>
                      {agentLabel(entry.source)}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            {detectLoadState === 'ready' && detectedConnectors.length === 0 ? (
              <p className={styles.caveat} role="status">
                No configuration files found yet. You can continue and connect agents later from
                Integrations.
              </p>
            ) : null}
          </>
        ) : null}

        {step === 1 ? (
          <>
            <p className={styles.muted}>
              LLM Notch lives in a compact island overlay on your chosen display. Pick where the
              island should appear — no account required.
            </p>
            <div className={styles.field}>
              <label htmlFor="onboarding-display">
                <Monitor size={14} aria-hidden="true" /> Display
              </label>
              <select
                id="onboarding-display"
                className={styles.select}
                value={selectedDisplayId ?? AUTOMATIC_DISPLAY_VALUE}
                onChange={(event) =>
                  onDisplayChange(
                    event.target.value === AUTOMATIC_DISPLAY_VALUE ? null : event.target.value,
                  )
                }
              >
                <option value={AUTOMATIC_DISPLAY_VALUE}>Automatic (current/primary display)</option>
                {displays.map((display) => (
                  <option key={display.id} value={display.id}>
                    {display.label}
                    {display.primary ? ' (primary)' : ''}
                  </option>
                ))}
              </select>
              {displayLoadState === 'loading' ? (
                <p className={styles.muted} role="status">
                  Detecting displays…
                </p>
              ) : null}
              {displayLoadState === 'error' ? (
                <p className={styles.caveat} role="alert">
                  {displayError ??
                    'Display enumeration failed; automatic display remains available.'}
                </p>
              ) : null}
            </div>
            {!fullscreenPreferenceSupported ? (
              <p className={styles.caveat}>
                Windows supports the normal topmost island, but not overlay presentation above
                fullscreen applications.
              </p>
            ) : null}
          </>
        ) : null}

        {step === 2 ? (
          <>
            <p className={styles.muted}>
              Select which agent files to connect. User scope is the default — project scope is
              optional for team-shared configuration. Every supported agent is preselected.
            </p>
            <div className={styles.scopeToggle} role="radiogroup" aria-label="Connector scope">
              <button
                type="button"
                role="radio"
                aria-checked={connectScope === 'user'}
                className={
                  connectScope === 'user' ? styles.scopeOptionActive : styles.scopeOption
                }
                onClick={() => onConnectScopeChange('user')}
              >
                User scope
                <span className={styles.scopeHint}>Recommended</span>
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={connectScope === 'project'}
                className={
                  connectScope === 'project' ? styles.scopeOptionActive : styles.scopeOption
                }
                onClick={() => onConnectScopeChange('project')}
              >
                Project scope
              </button>
            </div>
            <fieldset className={styles.fieldsetReset}>
              <legend className={styles.metricLabel}>Connect agents</legend>
              <ul className={styles.agentSelectList}>
                {integrationOptions.map((source) => {
                  const paths = connectSelections.filter((entry) => entry.source === source)
                  if (paths.length === 0) {
                    const detected = detectedConnectors.find((entry) => entry.source === source)
                    if (!detected) return null
                    return (
                      <li key={source}>
                        <label className={styles.agentSelectCard}>
                          <input
                            type="checkbox"
                            checked={false}
                            onChange={() =>
                              onConnectSelectionChange([
                                ...connectSelections,
                                {
                                  source,
                                  displayPath: detected.displayPath,
                                  selected: true,
                                },
                              ])
                            }
                          />
                          <span className={styles.agentSelectBody}>
                            <span className={styles.agentSelectName}>{agentLabel(source)}</span>
                            <span className={styles.mono}>{detected.displayPath}</span>
                          </span>
                        </label>
                      </li>
                    )
                  }
                  return paths.map((entry) => (
                    <li key={`${entry.source}-${entry.displayPath}`}>
                      <label className={styles.agentSelectCard}>
                        <input
                          type="checkbox"
                          checked={entry.selected}
                          onChange={(event) =>
                            onConnectSelectionChange(
                              connectSelections.map((current) =>
                                current.displayPath === entry.displayPath &&
                                current.source === entry.source
                                  ? { ...current, selected: event.target.checked }
                                  : current,
                              ),
                            )
                          }
                        />
                        <span className={styles.agentSelectBody}>
                          <span className={styles.agentSelectName}>{agentLabel(entry.source)}</span>
                          <span className={styles.mono}>{entry.displayPath}</span>
                        </span>
                      </label>
                    </li>
                  ))
                })}
              </ul>
            </fieldset>
            <p className={styles.caveat}>
              <Sparkles size={12} aria-hidden="true" /> One confirmation connects every selected
              agent. Unrelated hooks are preserved and backups are created before each write.
            </p>
          </>
        ) : null}

        {step === 3 ? (
          <>
            {pendingPlanCount > 1 ? (
              <p className={styles.muted} role="status">
                One confirmation will apply {pendingPlanCount} vendor plans sequentially with
                per-file results below.
              </p>
            ) : null}
            {pendingPlan ? (
              <DiffReviewPanel
                plan={pendingPlan.plan}
                selectedFilePaths={pendingPlan.selectedFilePaths}
                onToggleFile={(displayPath, selected) => {
                  onTogglePlanFile?.(displayPath, selected)
                  onConnectSelectionChange(
                    connectSelections.map((entry) =>
                      entry.displayPath === displayPath ? { ...entry, selected } : entry,
                    ),
                  )
                }}
                onConfirm={onConfirmApply}
                onCancel={onSkipConnect}
              />
            ) : null}
            {applyProgress && applyProgress.length > 0 ? (
              <ApplyProgressPanel progress={applyProgress} result={applyResult} />
            ) : null}
            {!pendingPlan && (!applyProgress || applyProgress.length === 0) ? (
              <p className={styles.muted}>No files selected — continue to shortcuts.</p>
            ) : null}
          </>
        ) : null}

        {step === 4 ? (
          <>
            <p className={styles.muted}>
              Toggle the dashboard with <strong>{shortcutLabel}</strong>. Optional autostart keeps
              the helper ready at login.
            </p>
            <label className={styles.toggleCard}>
              <input
                type="checkbox"
                checked={autostartEnabled}
                onChange={(event) => onAutostartChange(event.target.checked)}
              />
              <span className={styles.toggleCardBody}>
                <Zap size={16} aria-hidden="true" />
                <span>
                  <span className={styles.toggleCardTitle}>Launch at startup</span>
                  <span className={styles.toggleCardHint}>Keep LLM Notch ready after login</span>
                </span>
              </span>
            </label>
          </>
        ) : null}

        {confirmSkip ? (
          <div className={styles.confirmDialog} role="alertdialog" aria-label="Confirm skip setup">
            <p className={styles.muted}>
              Skip onboarding? You can configure displays and integrations later in Settings.
            </p>
            <div className={styles.actions}>
              <button type="button" className={styles.button} onClick={() => setConfirmSkip(false)}>
                Continue setup
              </button>
              <button type="button" className={styles.buttonDanger} onClick={onSkip}>
                Confirm skip
              </button>
            </div>
          </div>
        ) : (
          <div className={styles.actions}>
            <button
              type="button"
              className={styles.buttonGhost}
              onClick={() => setConfirmSkip(true)}
            >
              Skip setup
            </button>
            {step > 0 ? (
              <button type="button" className={styles.button} onClick={onBack}>
                Back
              </button>
            ) : null}
            {step === 0 && detectLoadState !== 'ready' ? (
              <button
                type="button"
                className={styles.buttonPrimary}
                onClick={onGetStarted}
                disabled={detectLoadState === 'loading'}
              >
                {detectLoadState === 'loading' ? (
                  <>
                    <Loader2 size={14} className={styles.spinIcon} aria-hidden="true" />
                    Detecting…
                  </>
                ) : (
                  <>
                    <Radar size={14} aria-hidden="true" />
                    Detect all agents
                  </>
                )}
              </button>
            ) : null}
            {step === 0 && detectLoadState === 'ready' ? (
              <button type="button" className={styles.buttonPrimary} onClick={onNext}>
                Continue
              </button>
            ) : null}
            {step === 2 ? (
              <>
                <button type="button" className={styles.button} onClick={onSkipConnect}>
                  Skip connect
                </button>
                <button
                  type="button"
                  className={styles.buttonPrimary}
                  onClick={onPreviewConnect}
                  disabled={selectedCount === 0}
                >
                  Review & connect ({selectedCount})
                </button>
              </>
            ) : null}
            {step !== 0 && step !== 2 && step !== 3 ? (
              <button
                type="button"
                className={styles.buttonPrimary}
                onClick={isLastStep ? onFinish : onNext}
              >
                {isLastStep ? 'Finish' : 'Continue'}
              </button>
            ) : null}
            {step === 3 && !pendingPlan ? (
              <button type="button" className={styles.buttonPrimary} onClick={onNext}>
                Continue
              </button>
            ) : null}
          </div>
        )}
      </div>
    </div>
  )
}
