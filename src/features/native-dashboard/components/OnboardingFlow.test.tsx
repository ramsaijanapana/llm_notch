import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { AgentSource } from '../../../native/contracts'
import { mockDisplays } from '../fixtures/testFixtures'
import { OnboardingFlow } from './OnboardingFlow'

const integrationOptions: AgentSource[] = ['cursor', 'claudeCode', 'codex', 'generic']

const baseProps = {
  open: true,
  step: 0 as const,
  displays: mockDisplays,
  selectedDisplayId: 'display-primary',
  onDisplayChange: vi.fn(),
  integrationOptions,
  selectedIntegration: 'none' as const,
  onIntegrationChange: vi.fn(),
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

  it('renders step one display selection', () => {
    render(<OnboardingFlow {...baseProps} />)
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: /^display$/i })).toHaveFocus()
  })

  it('advances through steps and can finish', async () => {
    const user = userEvent.setup()
    const onNext = vi.fn()
    const onFinish = vi.fn()

    const { rerender } = render(
      <OnboardingFlow {...baseProps} onNext={onNext} onFinish={onFinish} />,
    )
    await user.click(screen.getByRole('button', { name: /continue/i }))
    expect(onNext).toHaveBeenCalled()

    rerender(<OnboardingFlow {...baseProps} step={1} onNext={onNext} onFinish={onFinish} />)
    expect(
      screen.getByText(/never writes vendor configuration files automatically/i),
    ).toBeInTheDocument()

    rerender(<OnboardingFlow {...baseProps} step={2} onNext={onNext} onFinish={onFinish} />)
    await user.click(screen.getByRole('button', { name: /finish/i }))
    expect(onFinish).toHaveBeenCalled()
  })

  it('is skippable without account requirement', async () => {
    const user = userEvent.setup()
    const onSkip = vi.fn()

    render(<OnboardingFlow {...baseProps} onSkip={onSkip} />)
    await user.click(screen.getByRole('button', { name: /skip setup/i }))
    expect(screen.getByRole('alertdialog', { name: /confirm skip setup/i })).toBeInTheDocument()
    expect(onSkip).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: /confirm skip/i }))
    expect(onSkip).toHaveBeenCalled()
  })

  it('traps focus and uses Escape confirmation', async () => {
    const user = userEvent.setup()
    render(<OnboardingFlow {...baseProps} />)
    const display = screen.getByRole('combobox', { name: /^display$/i })
    expect(display).toHaveFocus()

    await user.tab({ shift: true })
    expect(screen.getByRole('button', { name: /continue/i })).toHaveFocus()
    await user.tab()
    expect(display).toHaveFocus()

    await user.keyboard('{Escape}')
    expect(screen.getByRole('alertdialog', { name: /confirm skip setup/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /continue setup/i })).toHaveFocus()
  })

  it('restores focus after closing', () => {
    const trigger = document.createElement('button')
    trigger.textContent = 'Open setup'
    document.body.append(trigger)
    trigger.focus()
    const { rerender } = render(<OnboardingFlow {...baseProps} />)
    expect(screen.getByRole('combobox', { name: /^display$/i })).toHaveFocus()
    rerender(<OnboardingFlow {...baseProps} open={false} />)
    expect(trigger).toHaveFocus()
    trigger.remove()
  })

  it('does not render when closed', () => {
    render(<OnboardingFlow {...baseProps} open={false} />)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('explains Windows fullscreen limitation', () => {
    render(<OnboardingFlow {...baseProps} fullscreenPreferenceSupported={false} />)
    expect(screen.getByText(/not overlay presentation above fullscreen/i)).toBeInTheDocument()
  })
})
