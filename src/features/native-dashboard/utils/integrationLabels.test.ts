import { describe, expect, it } from 'vitest'
import {
  connectorStatusGuidance,
  connectorStatusLabel,
  decisionDeliveryLabel,
  DOCUMENTED_CONNECTOR_PATHS,
} from './integrationLabels'

describe('integrationLabels', () => {
  it('documents all seven verified connector paths', () => {
    expect(DOCUMENTED_CONNECTOR_PATHS.map((entry) => entry.source)).toEqual([
      'cursor',
      'claudeCode',
      'codex',
      'gemini',
      'qwen',
      'antigravityCli',
      'copilotCli',
    ])
    expect(DOCUMENTED_CONNECTOR_PATHS.find((entry) => entry.source === 'qwen')?.userPath).toBe(
      '~/.qwen/settings.json',
    )
    expect(
      DOCUMENTED_CONNECTOR_PATHS.find((entry) => entry.source === 'antigravityCli')?.projectPath,
    ).toBe('<repo>/.agents/hooks.json')
    expect(
      DOCUMENTED_CONNECTOR_PATHS.find((entry) => entry.source === 'copilotCli')?.userPath,
    ).toBe('~/.copilot/hooks/llm-notch.json')
  })

  it('maps ConnectorUserStatus to labels', () => {
    expect(connectorStatusLabel('connected')).toBe('Connected')
    expect(connectorStatusLabel('actionNeeded')).toBe('Action needed')
  })

  it('provides Cursor hooks guidance', () => {
    expect(connectorStatusGuidance('cursor', 'actionNeeded')).toMatch(/cursor settings/i)
  })

  it('provides Codex /hooks guidance', () => {
    expect(connectorStatusGuidance('codex', 'actionNeeded')).toMatch(/\/hooks/i)
  })

  it('maps decision delivery states', () => {
    expect(decisionDeliveryLabel('pending')).toMatch(/pending delivery/i)
    expect(decisionDeliveryLabel('effectObserved')).toMatch(/acknowledged/i)
  })
})
