import { Footer, Header } from '../components'
import { NotchDemo } from '../features/notch-sim/components'
import { SimulationProvider } from '../features/notch-sim/model/SimulationProvider'
import { Capabilities, FAQ, FinalCta, Hero, LocalFirst, Pricing, Workflow } from '../sections'
import styles from './App.module.css'

export default function App() {
  return (
    <SimulationProvider>
      <div className={styles.shell}>
        <a href="#main-content" className={`sr-only sr-only-focusable ${styles.skipLink}`}>
          Skip to main content
        </a>

        <Header />

        <main id="main-content" className={styles.main} tabIndex={-1}>
          <Hero />
          <NotchDemo />
          <Workflow />
          <Capabilities />
          <LocalFirst />
          <Pricing />
          <FAQ />
          <FinalCta />
        </main>

        <Footer />
      </div>
    </SimulationProvider>
  )
}
