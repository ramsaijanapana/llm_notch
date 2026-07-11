import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mockDisplays, mockSettings } from '../../fixtures/testFixtures'
import { SettingsPanel } from './SettingsPanel'

describe('SettingsPanel', () => {
  afterEach(() => cleanup())

  it('renders overlay, shortcut, sampling, and privacy controls', () => {
    render(
      <SettingsPanel
        settings={mockSettings}
        displays={mockDisplays}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={vi.fn()}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )

    expect(screen.getByLabelText(/show island overlay/i)).toBeChecked()
    expect(screen.getByText(/ctrl\+shift\+n/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/sampling interval/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/history retention/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/reduce motion/i)).toBeInTheDocument()
  })

  it('emits settings patches on change', async () => {
    const user = userEvent.setup()
    const onSettingsChange = vi.fn()
    const onDisplayChange = vi.fn()

    render(
      <SettingsPanel
        settings={mockSettings}
        displays={mockDisplays}
        onDisplayChange={onDisplayChange}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={onSettingsChange}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )

    await user.click(screen.getByLabelText(/launch at startup/i))
    expect(onSettingsChange).toHaveBeenCalledWith({ autostartEnabled: true })
    await user.selectOptions(screen.getByRole('combobox', { name: /^display$/i }), '__automatic__')
    expect(onDisplayChange).toHaveBeenCalledWith(null)
  })

  it('shows an explicit display enumeration error', () => {
    render(
      <SettingsPanel
        settings={mockSettings}
        displays={[]}
        displayLoadState="error"
        displayError="Monitor API unavailable"
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={vi.fn()}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )
    expect(screen.getByRole('alert')).toHaveTextContent('Monitor API unavailable')
  })

  it('disables unsupported Windows fullscreen preference honestly', () => {
    render(
      <SettingsPanel
        settings={mockSettings}
        displays={mockDisplays}
        fullscreenPreferenceSupported={false}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={vi.fn()}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )
    expect(screen.getByLabelText(/show over fullscreen apps/i)).toBeDisabled()
    expect(screen.getByText(/fullscreen overlay is unavailable on windows/i)).toBeInTheDocument()
  })

  it('requires confirmation before purge', async () => {
    const user = userEvent.setup()
    const onPurgeHistory = vi.fn()
    const onPurgeConfirm = vi.fn()

    const { rerender } = render(
      <SettingsPanel
        settings={mockSettings}
        displays={mockDisplays}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={vi.fn()}
        onPurgeHistory={onPurgeHistory}
        purgeConfirmOpen={false}
        onPurgeConfirm={onPurgeConfirm}
        onPurgeCancel={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: /purge history now/i }))
    expect(onPurgeHistory).toHaveBeenCalled()

    rerender(
      <SettingsPanel
        settings={mockSettings}
        displays={mockDisplays}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={vi.fn()}
        onPurgeHistory={onPurgeHistory}
        purgeConfirmOpen
        onPurgeConfirm={onPurgeConfirm}
        onPurgeCancel={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: /^purge$/i }))
    expect(onPurgeConfirm).toHaveBeenCalled()
  })
})
