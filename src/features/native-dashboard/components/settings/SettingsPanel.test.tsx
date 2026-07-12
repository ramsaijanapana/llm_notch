import { cleanup, fireEvent, render, screen } from '@testing-library/react'
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

  it('renders master volume and quiet-hours controls', () => {
    render(
      <SettingsPanel
        settings={{
          ...mockSettings,
          soundRouting: {
            enabled: true,
            volume: 0.5,
            quietHours: { startMinute: 1_320, endMinute: 420 },
            eventVolume: {},
            agentVolume: {},
          },
        }}
        displays={mockDisplays}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={vi.fn()}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )

    expect(screen.getByLabelText(/master volume/i)).toHaveValue('50')
    expect(screen.getByLabelText(/quiet hours/i)).toBeChecked()
    expect(screen.getByLabelText(/^start$/i)).toHaveValue('22:00')
    expect(screen.getByLabelText(/^end$/i)).toHaveValue('07:00')
  })

  it('renders per-event and per-agent volume with honest 100% defaults', () => {
    render(
      <SettingsPanel
        settings={{
          ...mockSettings,
          soundRouting: {
            enabled: true,
            volume: 0.8,
            quietHours: null,
            eventVolume: { notification: 0.5 },
            agentVolume: { codex: 0.25 },
          },
        }}
        displays={mockDisplays}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={vi.fn()}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )

    expect(screen.getByLabelText(/notification \(50%\)/i)).toHaveValue('50')
    expect(screen.getByLabelText(/completed \(100%\)/i)).toHaveValue('100')
    expect(screen.getByLabelText(/codex \(25%\)/i)).toHaveValue('25')
    expect(screen.getByLabelText(/cursor \(100%\)/i)).toHaveValue('100')
    expect(screen.getByLabelText(/qwen code \(100%\)/i)).toHaveValue('100')
    expect(screen.getByLabelText(/antigravity cli \(100%\)/i)).toHaveValue('100')
    expect(screen.getByLabelText(/github copilot cli \(100%\)/i)).toHaveValue('100')
  })

  it('emits soundRouting patches for per-event and per-agent volume', async () => {
    const onSettingsChange = vi.fn()

    render(
      <SettingsPanel
        settings={{
          ...mockSettings,
          soundRouting: {
            enabled: true,
            volume: 0.8,
            quietHours: null,
            eventVolume: {},
            agentVolume: {},
          },
        }}
        displays={mockDisplays}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={onSettingsChange}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )

    fireEvent.change(screen.getByLabelText(/approval \(100%\)/i), { target: { value: '80' } })
    expect(onSettingsChange).toHaveBeenLastCalledWith({
      soundRouting: {
        enabled: true,
        volume: 0.8,
        quietHours: null,
        eventVolume: { approval: 0.8 },
        agentVolume: {},
      },
    })

    fireEvent.change(screen.getByLabelText(/codex \(100%\)/i), { target: { value: '90' } })
    expect(onSettingsChange).toHaveBeenLastCalledWith({
      soundRouting: {
        enabled: true,
        volume: 0.8,
        quietHours: null,
        eventVolume: {},
        agentVolume: { codex: 0.9 },
      },
    })
  })

  it('clears per-event override when volume returns to 100%', () => {
    const onSettingsChange = vi.fn()

    render(
      <SettingsPanel
        settings={{
          ...mockSettings,
          soundRouting: {
            enabled: true,
            volume: 0.8,
            quietHours: null,
            eventVolume: { failed: 0.5 },
            agentVolume: {},
          },
        }}
        displays={mockDisplays}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={onSettingsChange}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )

    fireEvent.change(screen.getByLabelText(/failed \(50%\)/i), { target: { value: '100' } })
    expect(onSettingsChange).toHaveBeenLastCalledWith({
      soundRouting: {
        enabled: true,
        volume: 0.8,
        quietHours: null,
        eventVolume: {},
        agentVolume: {},
      },
    })
  })

  it('clears per-agent override when volume returns to 100%', () => {
    const onSettingsChange = vi.fn()

    render(
      <SettingsPanel
        settings={{
          ...mockSettings,
          soundRouting: {
            enabled: true,
            volume: 0.8,
            quietHours: null,
            eventVolume: {},
            agentVolume: { gemini: 0.5 },
          },
        }}
        displays={mockDisplays}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={onSettingsChange}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )

    fireEvent.change(screen.getByLabelText(/gemini cli \(50%\)/i), { target: { value: '100' } })
    expect(onSettingsChange).toHaveBeenLastCalledWith({
      soundRouting: {
        enabled: true,
        volume: 0.8,
        quietHours: null,
        eventVolume: {},
        agentVolume: {},
      },
    })
  })

  it('shows playback caveat when native sound is unavailable', () => {
    render(
      <SettingsPanel
        settings={mockSettings}
        displays={mockDisplays}
        soundPlaybackSupported={false}
        onDisplayChange={vi.fn()}
        shortcutLabel="Ctrl+Shift+N"
        onSettingsChange={vi.fn()}
        onPurgeHistory={vi.fn()}
        onPurgeConfirm={vi.fn()}
        onPurgeCancel={vi.fn()}
      />,
    )

    expect(
      screen.getByText(/native alert sounds are unavailable on this platform/i),
    ).toBeInTheDocument()
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
