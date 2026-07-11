import { useState } from 'react'
import { siteContent } from '../../data/siteContent'
import { useDemoAnchorClick } from '../../features/notch-sim/model/useDemoAnchor'
import { ButtonLink } from '../ui/ButtonLink'
import styles from './Header.module.css'

function LogoMark() {
  return (
    <svg className={styles.logoMark} viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <rect
        x="4"
        y="4"
        width="24"
        height="24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <path d="M8 8v14h3V11h7V8H8zm14 0v14h3V11h-7V8h4z" fill="currentColor" />
      <path d="M22 22h6v3h-6z" fill="var(--color-amber)" />
    </svg>
  )
}

export function Header() {
  const { meta, header } = siteContent
  const [menuOpen, setMenuOpen] = useState(false)
  const handleDemoAnchorClick = useDemoAnchorClick()

  const handleNavLinkClick = (event: React.MouseEvent<HTMLAnchorElement>, href: string) => {
    setMenuOpen(false)

    if (href === '#demo') {
      handleDemoAnchorClick(event)
    }
  }

  return (
    <header className={styles.header}>
      <div className={styles.inner}>
        <a href="#main-content" className={styles.brand} aria-label={`${meta.productName} home`}>
          <LogoMark />
          <span className={styles.brandName}>{meta.productName}</span>
        </a>

        <button
          type="button"
          className={styles.menuToggle}
          aria-expanded={menuOpen}
          aria-controls="primary-navigation"
          onClick={() => setMenuOpen((open) => !open)}
        >
          Menu
        </button>

        <nav
          id="primary-navigation"
          className={`${styles.nav} ${menuOpen ? styles.navOpen : ''}`}
          aria-label="Primary"
        >
          <ul className={styles.navList}>
            {header.nav.map((link) => (
              <li key={link.href}>
                <a
                  href={link.href}
                  className={styles.navLink}
                  onClick={(event) => handleNavLinkClick(event, link.href)}
                >
                  {link.label}
                </a>
              </li>
            ))}
          </ul>
        </nav>

        <div className={styles.actions}>
          <ButtonLink
            href={header.cta.href}
            variant="primary"
            className={styles.cta}
            onClick={header.cta.href === '#demo' ? handleDemoAnchorClick : undefined}
          >
            {header.cta.label}
          </ButtonLink>
        </div>
      </div>
    </header>
  )
}
