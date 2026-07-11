import { type KeyboardEvent, useEffect, useRef, useState } from 'react'
import styles from '../styles/dashboard.module.css'
import type { OnboardingFlowProps } from '../types/contracts'
import { DOCUMENTED_CONNECTOR_PATHS } from '../utils/integrationLabels'
import { agentLabel } from '../utils/formatters'
import { ApplyProgressPanel } from './integrations/ApplyProgressPanel'
import { DiffReviewPanel } from './integrations/DiffReviewPanel'

const STEP_TITLES = [
  'Welcome',
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
    if (!open) setConfirmSkip(false)
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
      <div className={styles.onboardingCard}>
        <div className={styles.stepIndicator} aria-hidden="true">
          {STEP_TITLES.map((_, index) => (
            <span
              key={STEP_TITLES[index]}
              className={index === step ? styles.stepDotActive : styles.stepDot}
            />
          ))}
        </div>

        <h2 id="onboarding-title" className={styles.cardTitle}>
          {STEP_TITLES[step]}
        </h2>

        {step === 0 ? (
          <>
            <p className={styles.muted}>
              LLM Notch connects to your installed agents using only documented configuration paths.
              We never scan your disk — detection checks these fixed locations after you continue.
            </p>
            <ul className={styles.list}>
              {DOCUMENTED_CONNECTOR_PATHS.map((entry) => (
                <li key={entry.source} className={styles.muted}>
                  <strong>{agentLabel(entry.source)}</strong>: {entry.userPath}
                  {entry.source !== 'generic' ? ` (project: ${entry.projectPath})` : ''}
                </li>
              ))}
            </ul>
            {detectLoadState === 'loading' ? (
              <p className={styles.muted} role="status">
                Scanning documented paths…
              </p>
            ) : null}
            {detectLoadState === 'error' ? (
              <p className={styles.caveat} role="alert">
                {detectError ?? 'Detection failed. You can retry or skip and connect later.'}
              </p>
            ) : null}
            {detectLoadState === 'ready' && detectedConnectors.length > 0 ? (
              <p className={styles.muted} role="status">
                Found {detectedConnectors.length} configuration path
                {detectedConnectors.length === 1 ? '' : 's'}.
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
              <label htmlFor="onboarding-display">Display</label>
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
              optional for team-shared configuration.
            </p>
            <div className={styles.field}>
              <span className={styles.metricLabel}>Scope</span>
              <label className={styles.checkboxRow}>
                <input
                  type="radio"
                  name="connect-scope"
                  checked={connectScope === 'user'}
                  onChange={() => onConnectScopeChange('user')}
                />
                User scope (recommended)
              </label>
              <label className={styles.checkboxRow}>
                <input
                  type="radio"
                  name="connect-scope"
                  checked={connectScope === 'project'}
                  onChange={() => onConnectScopeChange('project')}
                />
                Project scope
              </label>
            </div>
            <fieldset className={styles.fieldsetReset}>
              <legend className={styles.metricLabel}>Connect agents</legend>
              {integrationOptions.map((source) => {
                const paths = connectSelections.filter((entry) => entry.source === source)
                if (paths.length === 0) {
                  const detected = detectedConnectors.find((entry) => entry.source === source)
                  if (!detected) return null
                  return (
                    <label key={source} className={styles.checkboxRow}>
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
                      {agentLabel(source)} — {detected.displayPath}
                    </label>
                  )
                }
                return paths.map((entry) => (
                  <label key={`${entry.source}-${entry.displayPath}`} className={styles.checkboxRow}>
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
                    {agentLabel(entry.source)} — {entry.displayPath}
                  </label>
                ))
              })}
            </fieldset>
            <p className={styles.caveat}>
              One confirmation applies all selected files. Unrelated hooks are preserved; backups
              are created before each write.
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
            <label className={styles.checkboxRow}>
              <input
                type="checkbox"
                checked={autostartEnabled}
                onChange={(event) => onAutostartChange(event.target.checked)}
              />
              Launch LLM Notch at startup
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
                {detectLoadState === 'loading' ? 'Detecting…' : 'Get started'}
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
                  Review changes ({selectedCount})
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
