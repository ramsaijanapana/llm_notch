import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { SparklineChart } from './SparklineChart'

describe('SparklineChart', () => {
  afterEach(() => cleanup())

  it('renders an accessible chart with data', () => {
    render(
      <SparklineChart
        label="CPU"
        points={[
          { atMs: 1, value: 1 },
          { atMs: 2, value: 4 },
          { atMs: 3, value: 2 },
        ]}
      />,
    )

    expect(screen.getByRole('img', { name: /cpu/i })).toBeInTheDocument()
  })

  it('shows empty state label when no points', () => {
    render(<SparklineChart label="RSS" points={[]} />)
    expect(screen.getByRole('img', { name: /no data/i })).toBeInTheDocument()
  })

  it('positions points by timestamp over the fixed selected domain', () => {
    const { container } = render(
      <SparklineChart
        label="Partial history"
        width={200}
        domainStartMs={0}
        domainEndMs={100}
        points={[
          { atMs: 75, value: 1 },
          { atMs: 100, value: 2 },
        ]}
      />,
    )
    const path = container.querySelector('path')
    expect(path).toHaveAttribute('d', expect.stringContaining('M150.00'))
    expect(path).toHaveAttribute('d', expect.stringContaining('L200.00'))
  })
})
