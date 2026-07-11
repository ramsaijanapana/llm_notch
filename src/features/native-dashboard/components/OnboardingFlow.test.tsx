import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { AgentSource } from '../../../native/contracts'
import { mockDisplays } from '../fixtures/testFixtures'
import { OnboardingFlow } from './OnboardingFlow'

const integrationOptions: AgentSource[] = ['cursor', 'claudeCode', 'codex']

const baseProps = {
  open: true,
  step: 0 as const,
  displays: mockDisplays,
  selectedDisplayId: 'display-primary',
  onDisplayChange: vi.fn(),
  integrationOptions,
  detectedConnectors: [],
  detectLoadState: 'idle' as const,
  onGetStarted: vi.fn(),
  connectSelections: [],
  onConnectSelectionChange: vi.fn(),
  connectScope: 'user' as const,
  onConnectScopeChange: vi.fn(),
  onPreviewConnect: vi.fn(),
  onConfirmApply: vi.fn(),
  onSkipConnect: vi.fn(),
  shortcutLabel: 'Ctrl+Shift+N',
  autostartEnabled: false,
  onAutostartChange: vi.fn(),
  onNext: vi.fn(),
  onBack: vi.fn(),
  onSkip: vi.fn(),
  onFinish: vi.fn(),
}

describe('OnboardingFlow', () => {
  afterEach(() => cleanup())

  it('renders consent step with documented paths', () => {
    render(<OnboardingFlow {...baseProps} />)
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByText(/documented configuration paths/i)).toBeInTheDocument()
    expect(screen.getByText(/~\/.cursor\/hooks.json/i)).toBeInTheDocument()
  })

  it('triggers detection from Get started', async () => {
    const user = userEvent.setup()
    const onGetStarted = vi.fn()
    render(<OnboardingFlow {...baseProps} onGetStarted={onGetStarted} />)
    await user.click(screen.getByRole('button', { name: /get started/i }))
    expect(onGetStarted).toHaveBeenCalled()
  })

  it('advances through display and finish steps', async () => {
    const user = userEvent.setup()
    const onNext = vi.fn()
    const onFinish = vi.fn()

    const { rerender } = render(
      <OnboardingFlow
        {...baseProps}
        detectLoadState="ready"
        onNext={onNext}
        onFinish={onFinish}
      />,
    )
    await user.click(screen.getByRole('button', { name: /continue/i }))
    expect(onNext).toHaveBeenCalled()

    rerender(
      <OnboardingFlow {...baseProps} step={1} detectLoadState="ready" onNext={onNext} onFinish={onFinish} />,
    )
    expect(screen.getByRole('combobox', { name: /^display$/i })).toBeInTheDocument()

    rerender(
      <OnboardingFlow {...baseProps} step={4} detectLoadState="ready" onNext={onNext} onFinish={onFinish} />,
    )
    await user.click(screen.getByRole('button', { name: /finish/i }))
    expect(onFinish).toHaveBeenCalled()
  })

  it('is skippable without account requirement', async () => {
    const user = userEvent.setup()
    const onSkip = vi.fn()

    render(<OnboardingFlow {...baseProps} onSkip={onSkip} />)
    await user.click(screen.getByRole('button', { name: /skip setup/i }))
    expect(screen.getByRole('alertdialog', { name: /confirm skip setup/i })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /confirm skip/i }))
    expect(onSkip).toHaveBeenCalled()
  })

  it('shows connect agents step with scope radios', () => {
    render(
      <OnboardingFlow
        {...baseProps}
        step={2}
        detectedConnectors={[
          {
            source: 'cursor',
            scope: 'user',
            displayPath: '~/.cursor/hooks.json',
            configPresent: true,
            managedEntriesPresent: false,
          },
        ]}
        connectSelections={[
          {
            source: 'cursor',
            displayPath: '~/.cursor/hooks.json',
            selected: true,
          },
        ]}
      />,
    )
    expect(screen.getByRole('heading', { name: /connect agents/i })).toBeInTheDocument()
    expect(screen.getByLabelText(/user scope/i)).toBeChecked()
  })
})
