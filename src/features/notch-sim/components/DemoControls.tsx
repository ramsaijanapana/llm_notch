import { Pause, Play, RotateCcw } from 'lucide-react'
import { useSimulation } from '../model/SimulationProvider'
import styles from './notchDemo.module.css'

type DemoControlsProps = {
  onReset?: () => void
}

export function DemoControls({ onReset }: DemoControlsProps) {
  const { state, dispatch } = useSimulation()

  return (
    <div className={styles.controls}>
      <div className={styles.controlGroup}>
        <button
          type="button"
          className={`${styles.btn} ${styles.btnPrimary}`}
          onClick={() => dispatch({ type: 'TOGGLE_PLAYBACK' })}
          aria-pressed={state.playing}
        >
          {state.playing ? (
            <>
              <Pause size={16} aria-hidden="true" />
              Pause
            </>
          ) : (
            <>
              <Play size={16} aria-hidden="true" />
              Play
            </>
          )}
        </button>
      </div>

      <div className={styles.controlSpacer} aria-hidden="true" />

      <div className={styles.controlGroup}>
        <button
          type="button"
          className={`${styles.btn} ${styles.btnGhost}`}
          onClick={() => {
            onReset?.()
            dispatch({ type: 'RESET' })
          }}
        >
          <RotateCcw size={16} aria-hidden="true" />
          Reset simulation
        </button>
      </div>
    </div>
  )
}
