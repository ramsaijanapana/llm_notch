import { Section } from '../components/ui/Section'
import { SectionHeader } from '../components/ui/SectionHeader'
import { siteContent } from '../data/siteContent'
import styles from './FAQ.module.css'

export function FAQ() {
  const { faq } = siteContent

  return (
    <Section id={faq.id} variant="inset" coordinate={faq.coordinate} aria-labelledby="faq-title">
      <div className={styles.container}>
        <SectionHeader id="faq-title" eyebrow={faq.eyebrow} title={faq.title} />

        <div className={styles.list}>
          {faq.items.map((item) => (
            <details key={item.id} className={styles.item}>
              <summary className={styles.question}>{item.question}</summary>
              <p className={styles.answer}>{item.answer}</p>
            </details>
          ))}
        </div>
      </div>
    </Section>
  )
}
