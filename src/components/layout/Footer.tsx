import { siteContent } from '../../data/siteContent'
import styles from './Footer.module.css'

export function Footer() {
  const { meta, footer } = siteContent
  const year = new Date().getFullYear()

  return (
    <footer className={styles.footer}>
      <div className={styles.inner}>
        <div className={styles.primary}>
          <p className={styles.statement}>{footer.statement}</p>
          <p className={styles.prototypeNote}>{footer.prototypeNote}</p>
        </div>

        <nav className={styles.nav} aria-label="Footer">
          <ul className={styles.navList}>
            {footer.nav.map((link) => (
              <li key={link.href}>
                <a href={link.href} className={styles.navLink}>
                  {link.label}
                </a>
              </li>
            ))}
          </ul>
        </nav>

        <p className={styles.copyright}>
          <span className={styles.mono}>© {year}</span> {meta.productName}
        </p>
      </div>
    </footer>
  )
}
