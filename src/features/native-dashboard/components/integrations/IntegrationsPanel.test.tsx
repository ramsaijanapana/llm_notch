import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FIXED_NOW_MS, mockIntegrations } from '../../fixtures/testFixtures'
import { IntegrationsPanel } from './IntegrationsPanel'

describe('IntegrationsPanel', () => {
  afterEach(() => cleanup())

  it('renders integration cards with capability matrix', () => {
    render(
      <IntegrationsPanel
        integrations={mockIntegrations}
        onPreview={vi.fn()}
        onApply={vi.fn()}
        onRemove={vi.fn()}
        onConfirmDiff={vi.fn()}
        onCancelDiff={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(screen.getByLabelText('Cursor integration')).toBeInTheDocument()
    expect(screen.getByLabelText('Claude Code integration')).toBeInTheDocument()
    expect(screen.getAllByText(/notify only/i).length).toBeGreaterThan(0)
  })

  it('invokes preview, apply, and remove callbacks', () => {
    const onPreview = vi.fn()
    const onApply = vi.fn()
    const onRemove = vi.fn()

    render(
      <IntegrationsPanel
        integrations={mockIntegrations}
        writeActionsAvailable
        onPreview={onPreview}
        onApply={onApply}
        onRemove={onRemove}
        onConfirmDiff={vi.fn()}
        onCancelDiff={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    const previewButton = screen.getAllByRole('button', { name: /^preview$/i })[0]
    if (!previewButton) throw new Error('Preview button not found')
    fireEvent.click(previewButton)
    expect(onPreview).toHaveBeenCalledWith('cursor')

    const applyButton = screen.getAllByRole('button', { name: /^apply reviewed plan$/i })[0]
    if (!applyButton) throw new Error('Apply button not found')
    fireEvent.click(applyButton)
    expect(onApply).toHaveBeenCalledWith('cursor')

    const removeButton = screen.getAllByRole('button', { name: /^remove$/i })[0]
    if (!removeButton) throw new Error('Remove button not found')
    fireEvent.click(removeButton)
    expect(onRemove).toHaveBeenCalledWith('cursor')
  })

  it('shows diff confirmation without writing files', async () => {
    const user = userEvent.setup()
    const onConfirmDiff = vi.fn()
    const onCancelDiff = vi.fn()

    render(
      <IntegrationsPanel
        integrations={mockIntegrations}
        pendingDiff={{
          source: 'cursor',
          summary: 'Enable Cursor adapter',
          before: '{}',
          after: '{ "enabled": true }',
        }}
        onPreview={vi.fn()}
        onApply={vi.fn()}
        onRemove={vi.fn()}
        onConfirmDiff={onConfirmDiff}
        onCancelDiff={onCancelDiff}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(
      screen.getByText(/automatic connector file writes are not available/i),
    ).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /close preview/i }))
    expect(onConfirmDiff).toHaveBeenCalled()
  })
})
