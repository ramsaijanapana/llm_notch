import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import type { AgentSource } from '../../../../native/contracts'
import { AgentStatusRail } from './AgentStatusRail'

const agents = [
  { source: 'cursor' as AgentSource, status: 'connected' as const, activeSessions: 2 },
  { source: 'gemini' as AgentSource, status: 'notInstalled' as const, attentionSessions: 1 },
]

describe('AgentStatusRail', () => {
  afterEach(() => cleanup())

  it('renders compact agent status cards', () => {
    render(<AgentStatusRail agents={agents} />)

    expect(screen.getByRole('region', { name: /agent status/i })).toBeInTheDocument()
    expect(screen.getByLabelText(/cursor: connected/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/gemini cli: not installed/i)).toBeInTheDocument()
    expect(screen.getByText(/2 active/i)).toBeInTheDocument()
    expect(screen.getByText(/1 need attention/i)).toBeInTheDocument()
  })

  it('renders nothing when agents list is empty', () => {
    const { container } = render(<AgentStatusRail agents={[]} />)
    expect(container).toBeEmptyDOMElement()
  })
})
