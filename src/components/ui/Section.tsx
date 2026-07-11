import type { ReactNode } from 'react'
import styles from './Section.module.css'

type SectionVariant = 'default' | 'band' | 'inset'

type SectionProps = {
  id?: string
  children: ReactNode
  variant?: SectionVariant
  coordinate?: string
  className?: string
  'aria-labelledby'?: string
}

export function Section({
  id,
  children,
  variant = 'default',
  coordinate,
  className,
  'aria-labelledby': ariaLabelledBy,
}: SectionProps) {
  const classes = [styles.section, styles[variant], className].filter(Boolean).join(' ')

  return (
    <section id={id} className={classes} aria-labelledby={ariaLabelledBy}>
      {coordinate ? (
        <span className={styles.coordinate} aria-hidden="true">
          {coordinate}
        </span>
      ) : null}
      {children}
    </section>
  )
}
