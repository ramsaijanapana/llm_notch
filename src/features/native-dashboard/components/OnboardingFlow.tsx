import { type KeyboardEvent, useEffect, useRef, useState } from 'react'
import styles from '../styles/dashboard.module.css'
import type { OnboardingFlowProps } from '../types/contracts'
import { agentLabel } from '../utils/formatters'

const STEP_TITLES = ['Island & display', 'Choose integration', 'Shortcuts & startup'] as const
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
  selectedIntegration,
  onIntegrationChange,
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
      'select:not([disabled]), input:not([disabled]), button:not([disabled]), [tabindex]:not([tabindex="-1"])',
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
  const isLastStep = step === 2
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault()
      setConfirmSkip((current) => !current)
      return
    }
    if (event.key !== 'Tab') return
    const focusable = Array.from(
      event.currentTarget.querySelectorAll<HTMLElement>(
        'select:not([disabled]), input:not([disabled]), button:not([disabled]), [tabindex]:not([tabindex="-1"])',
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

        {step === 1 ? (
          <>
            <p className={styles.muted}>
              Choose an integration template to preview. This build never writes vendor
              configuration files automatically.
            </p>
            <div className={styles.field}>
              <label htmlFor="onboarding-integration">Integration</label>
              <select
                id="onboarding-integration"
                className={styles.select}
                value={selectedIntegration}
                onChange={(event) =>
                  onIntegrationChange(
                    event.target.value as OnboardingFlowProps['selectedIntegration'],
                  )
                }
              >
                <option value="none">Skip for now</option>
                {integrationOptions.map((source) => (
                  <option key={source} value={source}>
                    {agentLabel(source)}
                  </option>
                ))}
              </select>
            </div>
            {selectedIntegration !== 'none' ? (
              <p className={styles.caveat}>
                Preview only — review and apply the versioned template manually from the
                integrations directory.
              </p>
            ) : null}
          </>
        ) : null}

        {step === 2 ? (
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
            <button
              type="button"
              className={styles.buttonPrimary}
              onClick={isLastStep ? onFinish : onNext}
            >
              {isLastStep ? 'Finish' : 'Continue'}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
