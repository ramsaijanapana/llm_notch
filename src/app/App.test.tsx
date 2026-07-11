import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'
import { siteContent } from '../data/siteContent'
import App from './App'

describe('App', () => {
  afterEach(() => {
    cleanup()
  })

  it('renders exactly one product h1', () => {
    render(<App />)

    const headings = screen.getAllByRole('heading', { level: 1 })
    expect(headings).toHaveLength(1)
    expect(headings[0]).toHaveTextContent(siteContent.hero.title)
  })

  it('surfaces prototype honesty copy', () => {
    render(<App />)

    expect(screen.getByText(siteContent.footer.prototypeNote)).toBeInTheDocument()
    expect(screen.getByText(/simulated agent sessions/i)).toBeInTheDocument()
    expect(screen.getByText(siteContent.hero.trustLine)).toBeInTheDocument()
  })

  it('exposes major page landmarks and anchor targets', () => {
    render(<App />)

    expect(screen.getByRole('navigation', { name: 'Primary' })).toBeInTheDocument()
    expect(screen.getByRole('main')).toHaveAttribute('id', 'main-content')
    expect(screen.getByRole('contentinfo')).toBeInTheDocument()

    expect(screen.getByRole('link', { name: /skip to main content/i })).toHaveAttribute(
      'href',
      '#main-content',
    )

    expect(document.getElementById('demo')).toBeInTheDocument()
    expect(document.getElementById('workflow')).toBeInTheDocument()
    expect(document.getElementById('capabilities')).toBeInTheDocument()
    expect(document.getElementById('local-first')).toBeInTheDocument()
    expect(document.getElementById('faq')).toBeInTheDocument()
  })

  it('shares one simulation provider across hero telemetry and the demo', async () => {
    const user = userEvent.setup()

    render(<App />)

    const capsule = screen.getByRole('button', { name: /interactive demo console/i })
    expect(capsule).toHaveAttribute('aria-expanded', 'false')

    await user.click(screen.getByRole('button', { name: /open interactive demo/i }))

    expect(capsule).toHaveAttribute('aria-expanded', 'true')
  })
})
