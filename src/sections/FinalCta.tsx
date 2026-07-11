import { ButtonLink } from '../components/ui/ButtonLink'
import { Section } from '../components/ui/Section'
import { siteContent } from '../data/siteContent'
import { useDemoAnchorClick } from '../features/notch-sim/model/useDemoAnchor'
import styles from './FinalCta.module.css'

export function FinalCta() {
  const { finalCta } = siteContent
  const handleDemoAnchorClick = useDemoAnchorClick()

  return (
    <Section variant="inset" coordinate={finalCta.coordinate}>
      <div className={styles.container}>
        <h2 className={styles.title}>{finalCta.title}</h2>
        <p className={styles.description}>{finalCta.description}</p>
        <ButtonLink
          href={finalCta.cta.href}
          variant="primary"
          onClick={finalCta.cta.href === '#demo' ? handleDemoAnchorClick : undefined}
        >
          {finalCta.cta.label}
        </ButtonLink>
      </div>
    </Section>
  )
}
