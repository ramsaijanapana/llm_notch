import type { MouseEventHandler, ReactNode } from 'react'
import styles from './ButtonLink.module.css'

type ButtonLinkVariant = 'primary' | 'secondary' | 'ghost'

type ButtonLinkProps = {
  href: string
  children: ReactNode
  variant?: ButtonLinkVariant
  className?: string | undefined
  onClick?: MouseEventHandler<HTMLAnchorElement> | undefined
}

export function ButtonLink({
  href,
  children,
  variant = 'primary',
  className,
  onClick,
}: ButtonLinkProps) {
  const classes = [styles.button, styles[variant], className].filter(Boolean).join(' ')

  return (
    <a href={href} className={classes} onClick={onClick}>
      {children}
    </a>
  )
}
