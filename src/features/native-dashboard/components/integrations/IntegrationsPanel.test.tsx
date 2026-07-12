import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { PREVIEW_AGENT_CATALOG } from '../../../../native/fixtures'
import { FIXED_NOW_MS, mockIntegrations } from '../../fixtures/testFixtures'
import { IntegrationsPanel } from './IntegrationsPanel'

describe('IntegrationsPanel', () => {
  afterEach(() => cleanup())

  it('renders integration cards with honest installation layers', () => {
    render(
      <IntegrationsPanel
        integrations={mockIntegrations}
        detectedConnectors={[
          {
            source: 'cursor',
            scope: 'user',
            displayPath: '~/.cursor/hooks.json',
            configPresent: true,
            managedEntriesPresent: true,
            executablePresent: true,
          },
          {
            source: 'codex',
            scope: 'user',
            displayPath: '~/.codex/hooks.json',
            configPresent: false,
            managedEntriesPresent: false,
            executablePresent: true,
            executablePath: 'C:\\Users\\dev\\AppData\\Roaming\\npm\\codex.cmd',
          },
        ]}
        backups={[]}
        onConnect={vi.fn()}
        onRepair={vi.fn()}
        onDisable={vi.fn()}
        onConfirmPlan={vi.fn()}
        onCancelPlan={vi.fn()}
        onRestoreBackup={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(screen.getByLabelText('Cursor integration')).toBeInTheDocument()
    expect(screen.getAllByText(/CLI: Installed/i).length).toBeGreaterThan(0)
    expect(screen.getByText(/CLI installed — hooks missing/i)).toBeInTheDocument()
    expect(screen.getAllByText(/waiting for first event/i).length).toBeGreaterThan(0)
  })

  it('invokes connect, repair, and disable callbacks', () => {
    const onConnect = vi.fn()
    const onRepair = vi.fn()
    const onDisable = vi.fn()

    render(
      <IntegrationsPanel
        integrations={mockIntegrations}
        backups={[]}
        writeActionsAvailable
        onConnect={onConnect}
        onRepair={onRepair}
        onDisable={onDisable}
        onConfirmPlan={vi.fn()}
        onCancelPlan={vi.fn()}
        onRestoreBackup={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    fireEvent.click(screen.getAllByRole('button', { name: /^connect$/i })[0]!)
    expect(onConnect).toHaveBeenCalledWith('cursor')

    fireEvent.click(screen.getAllByRole('button', { name: /^repair$/i })[0]!)
    expect(onRepair).toHaveBeenCalledWith('cursor')

    fireEvent.click(screen.getAllByRole('button', { name: /^disable$/i })[0]!)
    expect(onDisable).toHaveBeenCalledWith('cursor')
  })

  it('renders catalog-only agents as planned without connector controls', () => {
    render(
      <IntegrationsPanel
        integrations={mockIntegrations}
        catalog={PREVIEW_AGENT_CATALOG}
        backups={[]}
        onConnect={vi.fn()}
        onRepair={vi.fn()}
        onDisable={vi.fn()}
        onConfirmPlan={vi.fn()}
        onCancelPlan={vi.fn()}
        onRestoreBackup={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    const planned = screen.getByRole('region', { name: /planned integrations/i })
    expect(screen.getByLabelText('OpenCode planned integration')).toBeInTheDocument()
    expect(screen.getAllByText(/catalog only/i).length).toBeGreaterThanOrEqual(18)
    expect(planned.querySelectorAll('button')).toHaveLength(0)
    expect(screen.getAllByRole('button', { name: /^connect$/i })).toHaveLength(4)
  })

  it('shows diff review for pending plan', async () => {
    const user = userEvent.setup()
    const onConfirmPlan = vi.fn()

    render(
      <IntegrationsPanel
        integrations={mockIntegrations}
        backups={[]}
        pendingPlan={{
          plan: {
            planId: 'plan-1',
            source: 'cursor',
            scope: 'user',
            summary: 'Connect Cursor',
            expiresAtMs: Date.now() + 60_000,
            files: [
              {
                displayPath: '~/.cursor/hooks.json',
                baselineSha256: 'abc',
                diffText: '+ llm_notch hook',
                foreignEntriesPreserved: ['other'],
                isNewFile: false,
              },
            ],
            externalTrustActions: [],
          },
          selectedFilePaths: ['~/.cursor/hooks.json'],
        }}
        writeActionsAvailable
        onConnect={vi.fn()}
        onRepair={vi.fn()}
        onDisable={vi.fn()}
        onConfirmPlan={onConfirmPlan}
        onCancelPlan={vi.fn()}
        onRestoreBackup={vi.fn()}
        nowMs={FIXED_NOW_MS}
      />,
    )

    expect(screen.getByLabelText(/integration diff review/i)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /apply reviewed plan/i }))
    expect(onConfirmPlan).toHaveBeenCalled()
  })
})
