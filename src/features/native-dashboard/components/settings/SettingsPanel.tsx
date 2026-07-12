import styles from '../../styles/dashboard.module.css'
import type { AgentSource, PurgeScope, SoundEvent, SoundRouting } from '../../../../native/contracts'
import type { SettingsPanelProps } from '../../types/contracts'
import { routingAgentLabel, soundEventLabel } from '../../utils/formatters'
import { LoadingState } from '../shared/LoadingState'

const DEFAULT_SOUND_ROUTING: SoundRouting = {
  enabled: true,
  volume: 0.8,
  quietHours: null,
  eventVolume: {},
  agentVolume: {},
}

const DEFAULT_PER_ROUTE_VOLUME = 1

const ROUTING_SOUND_EVENTS: SoundEvent[] = [
  'notification',
  'completed',
  'approval',
  'question',
  'failed',
]

const ROUTING_AGENT_SOURCES = [
  'cursor',
  'claudeCode',
  'codex',
  'gemini',
  'qwen',
  'antigravityCli',
  'copilotCli',
] as const satisfies readonly AgentSource[]

function soundRouting(settings: SettingsPanelProps['settings']): SoundRouting {
  return settings.soundRouting ?? DEFAULT_SOUND_ROUTING
}

function effectiveRouteVolume(volume: number | undefined): number {
  return volume ?? DEFAULT_PER_ROUTE_VOLUME
}

function updateEventVolume(
  eventVolume: Partial<Record<SoundEvent, number>>,
  event: SoundEvent,
  volume: number,
): Partial<Record<SoundEvent, number>> {
  if (volume === DEFAULT_PER_ROUTE_VOLUME) {
    const { [event]: _removed, ...rest } = eventVolume
    return rest
  }
  return { ...eventVolume, [event]: volume }
}

function updateAgentVolume(
  agentVolume: Record<string, number>,
  agent: string,
  volume: number,
): Record<string, number> {
  if (volume === DEFAULT_PER_ROUTE_VOLUME) {
    const { [agent]: _removed, ...rest } = agentVolume
    return rest
  }
  return { ...agentVolume, [agent]: volume }
}

function minuteToTimeValue(minute: number): string {
  const hours = Math.floor(minute / 60)
  const mins = minute % 60
  return `${String(hours).padStart(2, '0')}:${String(mins).padStart(2, '0')}`
}

function timeValueToMinute(value: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(value)
  if (!match) {
    return null
  }
  const hours = Number(match[1])
  const mins = Number(match[2])
  if (hours > 23 || mins > 59) {
    return null
  }
  return hours * 60 + mins
}

const PREVIEW_EVENT_PRIORITY: SoundEvent[] = [
  'notification',
  'completed',
  'approval',
  'question',
  'failed',
]

function previewEventForTheme(theme: { events: Partial<Record<SoundEvent, unknown>> }): SoundEvent {
  for (const event of PREVIEW_EVENT_PRIORITY) {
    if (theme.events[event]) {
      return event
    }
  }
  return 'notification'
}

const DEFAULT_PURGE_SCOPE: PurgeScope = {
  history: true,
  sessionEvents: true,
  connectorJournal: false,
  includeBackups: false,
}

const SAMPLING_OPTIONS = [1000, 2000, 5000, 10_000] as const
const RETENTION_OPTIONS = [1, 6, 24, 72, 168] as const
const AUTOMATIC_DISPLAY_VALUE = '__automatic__'
const DEFAULT_SOUND_THEME_VALUE = ''

export function SettingsPanel({
  settings,
  displays,
  displayLoadState = 'ready',
  displayError,
  fullscreenPreferenceSupported = true,
  onDisplayChange,
  shortcutLabel,
  onSettingsChange,
  purgeScope = DEFAULT_PURGE_SCOPE,
  onPurgeScopeChange,
  onPurgeHistory,
  purgeConfirmOpen = false,
  onPurgeConfirm,
  onPurgeCancel,
  loadState = 'ready',
  soundThemes = [],
  soundImportMessage,
  soundImportError,
  soundImportBusy = false,
  onImportSoundPack,
  onPreviewSoundTheme,
  soundPlaybackSupported = true,
}: SettingsPanelProps) {
  if (loadState === 'loading') {
    return <LoadingState label="Loading settings…" />
  }

  const routing = soundRouting(settings)

  const updateSoundRouting = (patch: Partial<SoundRouting>) => {
    onSettingsChange({
      soundRouting: {
        ...routing,
        ...patch,
      },
    })
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
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={settings.alertSoundEnabled ?? false}
            onChange={(event) => onSettingsChange({ alertSoundEnabled: event.target.checked })}
          />
          Play alert sound for attention, lifecycle, and sustained resource alerts (never
          activates windows)
        </label>
        <fieldset className={`${styles.actions} ${styles.fieldsetReset}`}>
          <legend className={styles.cardTitle}>Purge scope</legend>
          <label className={styles.checkboxRow}>
            <input
              type="checkbox"
              checked={purgeScope.history ?? true}
              onChange={(event) => onPurgeScopeChange?.({ history: event.target.checked })}
            />
            Metrics history
          </label>
          <label className={styles.checkboxRow}>
            <input
              type="checkbox"
              checked={purgeScope.sessionEvents ?? true}
              onChange={(event) => onPurgeScopeChange?.({ sessionEvents: event.target.checked })}
            />
            Session events
          </label>
          <label className={styles.checkboxRow}>
            <input
              type="checkbox"
              checked={purgeScope.connectorJournal ?? false}
              onChange={(event) => onPurgeScopeChange?.({ connectorJournal: event.target.checked })}
            />
            Connector journal
          </label>
          <label className={styles.checkboxRow}>
            <input
              type="checkbox"
              checked={purgeScope.includeBackups ?? false}
              onChange={(event) => onPurgeScopeChange?.({ includeBackups: event.target.checked })}
            />
            Include connector backups (unchecked by default — backups are kept unless you opt in)
          </label>
        </fieldset>
        <div className={styles.actions}>
          <button type="button" className={styles.buttonDanger} onClick={onPurgeHistory}>
            Purge history now
          </button>
        </div>
        {purgeConfirmOpen ? (
          <div className={styles.confirmDialog} role="alertdialog" aria-label="Confirm purge">
            <p className={styles.muted}>
              Delete selected local data using the purge scope above? Connector backup files stay
              on disk unless you check “Include connector backups” above. This cannot be undone.
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

      <section className={styles.card} aria-label="Sound themes">
        <h3 className={styles.cardTitle}>Sound themes</h3>
        {!soundPlaybackSupported ? (
          <p className={styles.caveat}>
            Native alert sounds are unavailable on this platform. Settings are saved, but playback
            is skipped.
          </p>
        ) : null}
        <div className={styles.field}>
          <label htmlFor="settings-sound-volume">
            Master volume ({Math.round(routing.volume * 100)}%)
          </label>
          <input
            id="settings-sound-volume"
            type="range"
            min={0}
            max={100}
            step={5}
            value={Math.round(routing.volume * 100)}
            onChange={(event) =>
              updateSoundRouting({ volume: Number(event.target.value) / 100 })
            }
          />
        </div>
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={routing.quietHours != null}
            onChange={(event) =>
              updateSoundRouting({
                quietHours: event.target.checked
                  ? { startMinute: 22 * 60, endMinute: 7 * 60 }
                  : null,
              })
            }
          />
          Quiet hours (mute alert sounds overnight)
        </label>
        {routing.quietHours ? (
          <div className={styles.actions}>
            <div className={styles.field}>
              <label htmlFor="settings-quiet-start">Start</label>
              <input
                id="settings-quiet-start"
                type="time"
                className={styles.select}
                value={minuteToTimeValue(routing.quietHours.startMinute)}
                onChange={(event) => {
                  const startMinute = timeValueToMinute(event.target.value)
                  if (startMinute == null || !routing.quietHours) {
                    return
                  }
                  updateSoundRouting({
                    quietHours: {
                      ...routing.quietHours,
                      startMinute,
                    },
                  })
                }}
              />
            </div>
            <div className={styles.field}>
              <label htmlFor="settings-quiet-end">End</label>
              <input
                id="settings-quiet-end"
                type="time"
                className={styles.select}
                value={minuteToTimeValue(routing.quietHours.endMinute)}
                onChange={(event) => {
                  const endMinute = timeValueToMinute(event.target.value)
                  if (endMinute == null || !routing.quietHours) {
                    return
                  }
                  updateSoundRouting({
                    quietHours: {
                      ...routing.quietHours,
                      endMinute,
                    },
                  })
                }}
              />
            </div>
          </div>
        ) : null}
        <fieldset className={`${styles.actions} ${styles.fieldsetReset}`}>
          <legend className={styles.cardTitle}>Per-event volume</legend>
          <p className={styles.muted}>
            Each event multiplies master volume. Unset events use 100%.
          </p>
          {ROUTING_SOUND_EVENTS.map((event) => {
            const effectiveVolume = effectiveRouteVolume(routing.eventVolume[event])
            return (
              <div key={event} className={styles.field}>
                <label htmlFor={`settings-event-volume-${event}`}>
                  {soundEventLabel(event)} ({Math.round(effectiveVolume * 100)}%)
                </label>
                <input
                  id={`settings-event-volume-${event}`}
                  type="range"
                  min={0}
                  max={100}
                  step={5}
                  value={Math.round(effectiveVolume * 100)}
                  onChange={(changeEvent) =>
                    updateSoundRouting({
                      eventVolume: updateEventVolume(
                        routing.eventVolume,
                        event,
                        Number(changeEvent.target.value) / 100,
                      ),
                    })
                  }
                />
              </div>
            )
          })}
        </fieldset>
        <fieldset className={`${styles.actions} ${styles.fieldsetReset}`}>
          <legend className={styles.cardTitle}>Per-agent volume</legend>
          <p className={styles.muted}>
            Each agent multiplies master volume. Unset agents use 100%.
          </p>
          {ROUTING_AGENT_SOURCES.map((agent) => {
            const effectiveVolume = effectiveRouteVolume(routing.agentVolume[agent])
            return (
              <div key={agent} className={styles.field}>
                <label htmlFor={`settings-agent-volume-${agent}`}>
                  {routingAgentLabel(agent)} ({Math.round(effectiveVolume * 100)}%)
                </label>
                <input
                  id={`settings-agent-volume-${agent}`}
                  type="range"
                  min={0}
                  max={100}
                  step={5}
                  value={Math.round(effectiveVolume * 100)}
                  onChange={(changeEvent) =>
                    updateSoundRouting({
                      agentVolume: updateAgentVolume(
                        routing.agentVolume,
                        agent,
                        Number(changeEvent.target.value) / 100,
                      ),
                    })
                  }
                />
              </div>
            )
          })}
        </fieldset>
        <p className={styles.muted}>
          Import signed-off community packs as zip archives with a validated integrity manifest.
          Built-in themes cannot be replaced.
        </p>
        {soundThemes.length > 0 ? (
          <div className={styles.field}>
            <label htmlFor="settings-sound-theme">Active sound theme</label>
            <select
              id="settings-sound-theme"
              className={styles.select}
              value={settings.selectedSoundThemeId ?? DEFAULT_SOUND_THEME_VALUE}
              onChange={(event) =>
                onSettingsChange({
                  selectedSoundThemeId:
                    event.target.value === DEFAULT_SOUND_THEME_VALUE
                      ? ''
                      : event.target.value,
                })
              }
            >
              <option value={DEFAULT_SOUND_THEME_VALUE}>Built-in (8-bit)</option>
              {soundThemes.map((theme) => (
                <option key={theme.id} value={theme.id}>
                  {theme.name}
                </option>
              ))}
            </select>
          </div>
        ) : null}
        {soundThemes.length > 0 ? (
          <ul className={styles.unsupportedList}>
            {soundThemes.map((theme) => (
              <li key={theme.id}>
                {theme.name} <span className={styles.mono}>({theme.id})</span>
                {onPreviewSoundTheme ? (
                  <>
                    {' '}
                    <button
                      type="button"
                      className={styles.buttonGhost}
                      onClick={() => onPreviewSoundTheme(theme.id, previewEventForTheme(theme))}
                    >
                      Preview
                    </button>
                  </>
                ) : null}
              </li>
            ))}
          </ul>
        ) : (
          <p className={styles.muted}>No sound themes loaded.</p>
        )}
        <div className={styles.actions}>
          <label className={styles.button}>
            Import sound pack
            <input
              type="file"
              accept=".zip,application/zip"
              hidden
              disabled={soundImportBusy || !onImportSoundPack}
              onChange={(event) => {
                const file = event.target.files?.[0]
                event.target.value = ''
                if (file && onImportSoundPack) {
                  onImportSoundPack(file)
                }
              }}
            />
          </label>
        </div>
        {soundImportBusy ? (
          <p className={styles.muted} role="status">
            Validating sound pack…
          </p>
        ) : null}
        {soundImportMessage ? (
          <p className={styles.muted} role="status">
            {soundImportMessage}
          </p>
        ) : null}
        {soundImportError ? (
          <p className={styles.caveat} role="alert">
            {soundImportError}
          </p>
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
