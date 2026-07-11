import { Section } from '../components/ui/Section'
import { SectionHeader } from '../components/ui/SectionHeader'
import { siteContent } from '../data/siteContent'
import styles from './Workflow.module.css'

export function Workflow() {
  const { workflow } = siteContent

  return (
    <Section id={workflow.id} coordinate={workflow.coordinate} aria-labelledby="workflow-title">
      <div className={styles.container}>
        <SectionHeader
          id="workflow-title"
          eyebrow={workflow.eyebrow}
          title={workflow.title}
          lead={workflow.lead}
        />

        <ol className={styles.steps}>
          {workflow.steps.map((step) => (
            <li key={step.step} className={styles.step}>
              <span className={styles.stepNumber} aria-hidden="true">
                {step.step}
              </span>
              <div className={styles.stepBody}>
                <h3 className={styles.stepTitle}>{step.title}</h3>
                <p className={styles.stepDescription}>{step.description}</p>
              </div>
            </li>
          ))}
        </ol>
      </div>
    </Section>
  )
}
