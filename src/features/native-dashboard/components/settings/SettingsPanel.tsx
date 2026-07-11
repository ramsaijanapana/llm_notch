import styles from '../../styles/dashboard.module.css'
import type { SettingsPanelProps } from '../../types/contracts'
import { LoadingState } from '../shared/LoadingState'

const SAMPLING_OPTIONS = [1000, 2000, 5000, 10_000] as const
const RETENTION_OPTIONS = [1, 6, 24, 72, 168] as const
const AUTOMATIC_DISPLAY_VALUE = '__automatic__'

export function SettingsPanel({
  settings,
  displays,
  displayLoadState = 'ready',
  displayError,
  fullscreenPreferenceSupported = true,
  onDisplayChange,
  shortcutLabel,
  onSettingsChange,
  onPurgeHistory,
  purgeConfirmOpen = false,
  onPurgeConfirm,
  onPurgeCancel,
  loadState = 'ready',
}: SettingsPanelProps) {
  if (loadState === 'loading') {
    return <LoadingState label="Loading settings…" />
  }

  return (
    <div className={styles.settingsGrid}>
      <section className={styles.card} aria-label="Overlay and display">
        <h3 className={styles.cardTitle}>Overlay & display</h3>
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={settings.overlayEnabled}
            onChange={(event) => onSettingsChange({ overlayEnabled: event.target.checked })}
          />
          Show island overlay
        </label>
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={settings.showOverFullscreen}
            disabled={!fullscreenPreferenceSupported}
            onChange={(event) => onSettingsChange({ showOverFullscreen: event.target.checked })}
          />
          Show over fullscreen apps
        </label>
        {!fullscreenPreferenceSupported ? (
          <p className={styles.caveat}>
            Fullscreen overlay is unavailable on Windows. Normal topmost overlay behavior remains
            enabled.
          </p>
        ) : null}
        <div className={styles.field}>
          <label htmlFor="settings-display">Display</label>
          <select
            id="settings-display"
            className={styles.select}
            value={settings.selectedDisplay ?? AUTOMATIC_DISPLAY_VALUE}
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
              {displayError ?? 'Display enumeration failed; automatic display remains available.'}
            </p>
          ) : null}
        </div>
      </section>

      <section className={styles.card} aria-label="Shortcuts and startup">
        <h3 className={styles.cardTitle}>Shortcuts & startup</h3>
        <p className={styles.muted}>
          Dashboard shortcut: <span className={styles.mono}>{shortcutLabel}</span>
        </p>
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={settings.autostartEnabled}
            onChange={(event) => onSettingsChange({ autostartEnabled: event.target.checked })}
          />
          Launch at startup
        </label>
      </section>

      <section className={styles.card} aria-label="Sampling and privacy">
        <h3 className={styles.cardTitle}>Sampling & privacy</h3>
        <div className={styles.field}>
          <label htmlFor="settings-sampling">Sampling interval</label>
          <select
            id="settings-sampling"
            className={styles.select}
            value={settings.samplingIntervalMs}
            onChange={(event) =>
              onSettingsChange({ samplingIntervalMs: Number(event.target.value) })
            }
          >
            {SAMPLING_OPTIONS.map((value) => (
              <option key={value} value={value}>
                {value / 1000}s
              </option>
            ))}
          </select>
        </div>
        <div className={styles.field}>
          <label htmlFor="settings-retention">History retention</label>
          <select
            id="settings-retention"
            className={styles.select}
            value={settings.historyRetentionHours}
            onChange={(event) =>
              onSettingsChange({ historyRetentionHours: Number(event.target.value) })
            }
          >
            {RETENTION_OPTIONS.map((hours) => (
              <option key={hours} value={hours}>
                {hours}h
              </option>
            ))}
          </select>
        </div>
        <p className={styles.caveat}>
          Metrics history is stored locally on this device. Retention controls how long samples are
          kept before automatic purge.
        </p>
        <div className={styles.actions}>
          <button type="button" className={styles.buttonDanger} onClick={onPurgeHistory}>
            Purge history now
          </button>
        </div>
        {purgeConfirmOpen ? (
          <div className={styles.confirmDialog} role="alertdialog" aria-label="Confirm purge">
            <p className={styles.muted}>
              Delete all stored metrics history? This cannot be undone.
            </p>
            <div className={styles.actions}>
              <button type="button" className={styles.button} onClick={onPurgeCancel}>
                Cancel
              </button>
              <button type="button" className={styles.buttonDanger} onClick={onPurgeConfirm}>
                Purge
              </button>
            </div>
          </div>
        ) : null}
      </section>

      <section className={styles.card} aria-label="Accessibility">
        <h3 className={styles.cardTitle}>Accessibility</h3>
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={settings.reducedMotion}
            onChange={(event) => onSettingsChange({ reducedMotion: event.target.checked })}
          />
          Reduce motion
        </label>
      </section>
    </div>
  )
}
