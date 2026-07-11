import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { createFakeNativeClient } from '../native/FakeNativeClient.ts'
import { DesktopApp } from './DesktopApp.tsx'

describe('DesktopApp', () => {
  afterEach(() => {
    cleanup()
  })

  it('labels preview mode clearly and renders native feature surfaces', async () => {
    render(<DesktopApp client={createFakeNativeClient()} />)

    expect(screen.getByText(/preview \/ test mode/i)).toBeInTheDocument()

    await waitFor(() => {
      expect(screen.getByTestId('overlay-shell')).toBeInTheDocument()
      expect(screen.getByTestId('dashboard-shell')).toBeInTheDocument()
    })
    const background = document.querySelector('[data-dashboard-background]')
    expect(background).toHaveAttribute('inert')
    expect(background).toHaveAttribute('aria-hidden', 'true')
  })

  it('announces loading state accessibly before bootstrap completes', () => {
    render(<DesktopApp client={createFakeNativeClient()} />)

    expect(screen.getByText(/loading native snapshot/i)).toBeInTheDocument()
  })
})
