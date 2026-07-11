import { describe, expect, it } from 'vitest'
import {
  connectorStatusGuidance,
  connectorStatusLabel,
  decisionDeliveryLabel,
} from './integrationLabels'

describe('integrationLabels', () => {
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
