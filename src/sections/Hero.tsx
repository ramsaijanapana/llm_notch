import { ButtonLink } from '../components/ui/ButtonLink'
import { siteContent } from '../data/siteContent'
import { MiniTelemetry } from '../features/notch-sim/components'
import { useDemoAnchorClick } from '../features/notch-sim/model/useDemoAnchor'
import styles from './Hero.module.css'

export function Hero() {
  const { hero } = siteContent
  const handleDemoAnchorClick = useDemoAnchorClick()

  return (
    <section className={styles.hero} aria-labelledby="hero-title">
      <span className={styles.coordinate} aria-hidden="true">
        {hero.coordinate}
      </span>

      <div className={styles.inner}>
        <div className={styles.copy}>
          <p className={styles.eyebrow}>{hero.eyebrow}</p>
          <h1 id="hero-title" className={styles.title}>
            {hero.title}
          </h1>
          <p className={styles.description}>{hero.description}</p>

          <div className={styles.actions}>
            <ButtonLink
              href={hero.primaryCta.href}
              variant="primary"
              onClick={hero.primaryCta.href === '#demo' ? handleDemoAnchorClick : undefined}
            >
              {hero.primaryCta.label}
            </ButtonLink>
            <ButtonLink href={hero.secondaryCta.href} variant="secondary">
              {hero.secondaryCta.label}
            </ButtonLink>
          </div>

          <p className={styles.trustLine}>{hero.trustLine}</p>
        </div>

        <aside className={styles.preview} aria-label="Simulated telemetry preview">
          <div className={styles.previewFrame}>
            <span className={styles.previewLabel} aria-hidden="true">
              READOUT
            </span>
            <MiniTelemetry />
          </div>
        </aside>
      </div>
    </section>
  )
}
