import { ButtonLink } from '../components/ui/ButtonLink'
import { Section } from '../components/ui/Section'
import { SectionHeader } from '../components/ui/SectionHeader'
import { siteContent } from '../data/siteContent'
import { useDemoAnchorClick } from '../features/notch-sim/model/useDemoAnchor'
import styles from './Pricing.module.css'

export function Pricing() {
  const { pricing } = siteContent
  const handleDemoAnchorClick = useDemoAnchorClick()

  return (
    <Section id={pricing.id} coordinate={pricing.coordinate} aria-labelledby="pricing-title">
      <div className={styles.container}>
        <div className={styles.card}>
          <SectionHeader
            id="pricing-title"
            eyebrow={pricing.eyebrow}
            title={pricing.title}
            lead={pricing.subtitle}
            align="center"
          />

          <p className={styles.note}>{pricing.note}</p>
          <p className={styles.disclosure}>{pricing.disclosure}</p>

          <div className={styles.cta}>
            <ButtonLink
              href={pricing.cta.href}
              variant="secondary"
              onClick={pricing.cta.href === '#demo' ? handleDemoAnchorClick : undefined}
            >
              {pricing.cta.label}
            </ButtonLink>
          </div>
        </div>
      </div>
    </Section>
  )
}
