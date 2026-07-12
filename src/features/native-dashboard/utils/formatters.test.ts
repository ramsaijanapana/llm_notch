import { describe, expect, it } from 'vitest'
import {
  agentLabel,
  formatBytes,
  formatPercent,
  formatRelativeTime,
  isModifierPressed,
  quotaObservedSummary,
  routingAgentLabel,
} from './formatters'

describe('native-dashboard formatters', () => {
  it('formats bytes and percents', () => {
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatPercent(12.345)).toBe('12.3%')
  })

  it('labels agents and relative times', () => {
    expect(agentLabel('claudeCode')).toBe('Claude Code')
    expect(agentLabel('gemini')).toBe('Gemini CLI')
    expect(agentLabel('qwen')).toBe('Qwen Code')
    expect(agentLabel('antigravityCli')).toBe('Antigravity CLI')
    expect(agentLabel('copilotCli')).toBe('GitHub Copilot CLI')
    expect(routingAgentLabel('antigravity-cli')).toBe('Antigravity CLI')
    expect(formatRelativeTime(1_000, 30_000)).toBe('just now')
    expect(formatRelativeTime(1_000, 3_700_000)).toBe('1h ago')
  })

  it('detects platform modifier keys', () => {
    expect(isModifierPressed({ metaKey: true, ctrlKey: false } as KeyboardEvent)).toBe(true)
    expect(isModifierPressed({ metaKey: false, ctrlKey: true } as KeyboardEvent)).toBe(true)
    expect(isModifierPressed({ metaKey: false, ctrlKey: false } as KeyboardEvent)).toBe(false)
  })

  it('summarizes quota freshness from observed timestamps', () => {
    const nowMs = 1_700_000_000_000
    expect(
      quotaObservedSummary(
        [
          {
            service: 'claude',
            displayName: 'Claude',
            availability: 'available',
            observedAtMs: nowMs - 2 * 60_000,
            freshness: 'fresh',
          },
          {
            service: 'codex',
            displayName: 'Codex',
            availability: 'unavailable',
          },
        ],
        nowMs,
      ),
    ).toEqual({
      observedAtMs: nowMs - 2 * 60_000,
      freshness: 'fresh',
    })
    expect(
      quotaObservedSummary(
        [
          {
            service: 'claude',
            displayName: 'Claude',
            availability: 'available',
            observedAtMs: nowMs - 20 * 60_000,
          },
        ],
        nowMs,
      ),
    ).toEqual({
      observedAtMs: nowMs - 20 * 60_000,
      freshness: 'stale',
    })
    expect(quotaObservedSummary([], nowMs)).toBeUndefined()
  })
})
