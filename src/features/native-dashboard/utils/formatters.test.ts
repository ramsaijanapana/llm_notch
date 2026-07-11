import { describe, expect, it } from 'vitest'
import {
  agentLabel,
  formatBytes,
  formatPercent,
  formatRelativeTime,
  isModifierPressed,
} from './formatters'

describe('native-dashboard formatters', () => {
  it('formats bytes and percents', () => {
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatPercent(12.345)).toBe('12.3%')
  })

  it('labels agents and relative times', () => {
    expect(agentLabel('claudeCode')).toBe('Claude Code')
    expect(formatRelativeTime(1_000, 30_000)).toBe('just now')
    expect(formatRelativeTime(1_000, 3_700_000)).toBe('1h ago')
  })

  it('detects platform modifier keys', () => {
    expect(isModifierPressed({ metaKey: true, ctrlKey: false } as KeyboardEvent)).toBe(true)
    expect(isModifierPressed({ metaKey: false, ctrlKey: true } as KeyboardEvent)).toBe(true)
    expect(isModifierPressed({ metaKey: false, ctrlKey: false } as KeyboardEvent)).toBe(false)
  })
})
