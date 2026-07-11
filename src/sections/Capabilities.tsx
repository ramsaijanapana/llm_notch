import { Section } from '../components/ui/Section'
import { SectionHeader } from '../components/ui/SectionHeader'
import { siteContent } from '../data/siteContent'
import styles from './Capabilities.module.css'

export function Capabilities() {
  const { capabilities } = siteContent

  return (
    <Section
      id={capabilities.id}
      variant="inset"
      coordinate={capabilities.coordinate}
      aria-labelledby="capabilities-title"
    >
      <div className={styles.container}>
        <SectionHeader
          id="capabilities-title"
          eyebrow={capabilities.eyebrow}
          title={capabilities.title}
          lead={capabilities.lead}
        />

        <ul className={styles.bento}>
          {capabilities.cards.map((card) => (
            <li key={card.id} className={`${styles.card} ${styles[card.layout]}`}>
              <span className={styles.microLabel} aria-hidden="true">
                {card.microLabel}
              </span>
              <h3 className={styles.cardTitle}>{card.title}</h3>
              <p className={styles.cardDescription}>{card.description}</p>
            </li>
          ))}
        </ul>
      </div>
    </Section>
  )
}
