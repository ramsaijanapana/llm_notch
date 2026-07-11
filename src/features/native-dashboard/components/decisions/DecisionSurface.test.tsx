import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mockAdapters } from '../../fixtures/testFixtures'
import { DecisionSurface } from './DecisionSurface'

const request = {
  id: 'dec-1',
  sessionId: 'sess-1',
  source: 'claudeCode' as const,
  kind: 'approval' as const,
  summary: 'Allow npm test',
  hasActionablePayload: true,
  createdAtMs: Date.now(),
}

describe('DecisionSurface', () => {
  afterEach(() => cleanup())

  it('hides allow/deny when decisionResponse is false', () => {
    render(
      <DecisionSurface
        request={request}
        adapter={mockAdapters.find((entry) => entry.source === 'claudeCode')}
      />,
    )
    expect(screen.queryByRole('button', { name: /^allow$/i })).not.toBeInTheDocument()
    expect(screen.getByText(/cannot receive in-app responses/i)).toBeInTheDocument()
  })

  it('hides controls when payload is not actionable', () => {
    const respondableAdapter = mockAdapters.find((entry) => entry.source === 'claudeCode')
    if (!respondableAdapter) throw new Error('missing adapter')
    render(
      <DecisionSurface
        request={{ ...request, hasActionablePayload: false }}
        adapter={{ ...respondableAdapter, decisionResponse: true }}
      />,
    )
    expect(screen.queryByRole('button', { name: /^allow$/i })).not.toBeInTheDocument()
    expect(screen.getByText(/waiting for agent payload/i)).toBeInTheDocument()
  })

  it('shows allow/deny when adapter supports responses', async () => {
    const user = userEvent.setup()
    const onAllow = vi.fn()
    const respondableAdapter = mockAdapters.find((entry) => entry.source === 'claudeCode')
    if (!respondableAdapter) throw new Error('missing adapter')
    render(
      <DecisionSurface
        request={request}
        adapter={{ ...respondableAdapter, decisionResponse: true }}
        onAllow={onAllow}
        onDeny={vi.fn()}
      />,
    )
    await user.click(screen.getByRole('button', { name: /^allow$/i }))
    expect(onAllow).toHaveBeenCalled()
  })

  it('shows delivery microcopy after response', () => {
    render(
      <DecisionSurface
        request={request}
        adapter={mockAdapters[1]}
        deliveryRecord={{
          requestId: request.id,
          response: { type: 'action', action: 'allow' },
          respondedAtMs: Date.now(),
          deliveryState: 'delivered',
        }}
      />,
    )
    expect(screen.getByText(/response delivered to agent/i)).toBeInTheDocument()
  })
})
