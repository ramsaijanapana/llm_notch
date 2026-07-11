import type { ReactNode } from 'react'
import styles from './SectionHeader.module.css'

type SectionHeaderProps = {
  eyebrow: string
  title: string
  lead?: string
  id?: string
  align?: 'start' | 'center'
  children?: ReactNode
}

export function SectionHeader({
  eyebrow,
  title,
  lead,
  id,
  align = 'start',
  children,
}: SectionHeaderProps) {
  const classes = [styles.header, styles[align]].join(' ')

  return (
    <header className={classes}>
      <p className={styles.eyebrow}>{eyebrow}</p>
      <h2 id={id} className={styles.title}>
        {title}
      </h2>
      {lead ? <p className={styles.lead}>{lead}</p> : null}
      {children}
    </header>
  )
}
