import { Section } from '../components/ui/Section'
import { SectionHeader } from '../components/ui/SectionHeader'
import { siteContent } from '../data/siteContent'
import styles from './LocalFirst.module.css'

export function LocalFirst() {
  const { localFirst } = siteContent

  return (
    <Section
      id={localFirst.id}
      variant="band"
      coordinate={localFirst.coordinate}
      aria-labelledby="local-first-title"
    >
      <div className={styles.container}>
        <SectionHeader
          id="local-first-title"
          eyebrow={localFirst.eyebrow}
          title={localFirst.title}
        />

        <div className={styles.body}>
          <div className={styles.copy}>
            {localFirst.paragraphs.map((paragraph) => (
              <p key={paragraph} className={styles.paragraph}>
                {paragraph}
              </p>
            ))}
          </div>

          <ul className={styles.bullets}>
            {localFirst.bullets.map((bullet) => (
              <li key={bullet} className={styles.bullet}>
                <span className={styles.bulletMark} aria-hidden="true">
                  +
                </span>
                {bullet}
              </li>
            ))}
          </ul>
        </div>
      </div>
    </Section>
  )
}
