import { describe, expect, it } from 'vitest'
import type { DetectedConnector } from '../../../native/contracts'
import {
  bestDetectedConnector,
  connectorInstallationLayers,
  connectorStatusGuidance,
  connectorStatusLabel,
  DOCUMENTED_CONNECTOR_PATHS,
  decisionDeliveryLabel,
  detectedConnectorSummary,
  effectiveConnectorStatusLabel,
  isDetectedConnectorVisible,
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
    expect(connectorStatusLabel('helperMissing')).toBe('Helper missing')
    expect(connectorStatusLabel('hooksMisconfigured')).toBe('Hooks misconfigured')
  })

  it('provides helper path guidance', () => {
    expect(connectorStatusGuidance('cursor', 'helperMissing')).toMatch(/repair/i)
    expect(connectorStatusGuidance('cursor', 'hooksMisconfigured')).toMatch(/bundled binary/i)
  })

  it('summarizes detected connectors with honest installation layers', () => {
    const cliOnly: DetectedConnector = {
      source: 'codex',
      scope: 'user',
      displayPath: '~/.codex/hooks.json',
      configPresent: false,
      managedEntriesPresent: false,
      executablePresent: true,
      executablePath: 'C:\\Users\\dev\\AppData\\Roaming\\npm\\codex.cmd',
    }
    const configDrift: DetectedConnector = {
      source: 'cursor',
      scope: 'user',
      displayPath: '~/.cursor/hooks.json',
      configPresent: true,
      managedEntriesPresent: false,
      executablePresent: true,
    }
    const connected: DetectedConnector = {
      source: 'cursor',
      scope: 'user',
      displayPath: '~/.cursor/hooks.json',
      configPresent: true,
      managedEntriesPresent: true,
      executablePresent: true,
    }

    expect(detectedConnectorSummary(cliOnly)).toMatch(/CLI installed.*hooks missing/i)
    expect(detectedConnectorSummary(configDrift)).toMatch(/hooks need repair/i)
    expect(detectedConnectorSummary(connected)).toBe('Hooks connected')
    expect(isDetectedConnectorVisible(cliOnly)).toBe(true)
    expect(
      isDetectedConnectorVisible({
        ...cliOnly,
        source: 'antigravityCli',
        executablePresent: false,
        configPresent: false,
      }),
    ).toBe(false)
  })

  it('renders installation layer labels separately', () => {
    const detected: DetectedConnector = {
      source: 'codex',
      scope: 'user',
      displayPath: '~/.codex/hooks.json',
      configPresent: false,
      managedEntriesPresent: false,
      executablePresent: true,
      executablePath: 'C:\\Users\\dev\\AppData\\Roaming\\npm\\codex.cmd',
    }
    const layers = connectorInstallationLayers(detected, 'notInstalled')
    expect(layers.cli).toMatch(/Installed/i)
    expect(layers.hookConfig).toBe('Not found')
    expect(layers.managedHooks).toMatch(/Connect/i)
    expect(layers.process).toBe('Not observed')
    expect(layers.traffic).toBe('No events yet')
  })

  it('prefers config-present detection when choosing best connector', () => {
    const detected: DetectedConnector[] = [
      {
        source: 'cursor',
        scope: 'project',
        displayPath: '<repo>/.cursor/hooks.json',
        configPresent: false,
        managedEntriesPresent: false,
        executablePresent: true,
      },
      {
        source: 'cursor',
        scope: 'user',
        displayPath: '~/.cursor/hooks.json',
        configPresent: true,
        managedEntriesPresent: false,
        executablePresent: true,
      },
    ]
    expect(bestDetectedConnector(detected, 'cursor')?.scope).toBe('user')
  })

  it('uses effective status labels for CLI-only installs', () => {
    const detected: DetectedConnector = {
      source: 'codex',
      scope: 'user',
      displayPath: '~/.codex/hooks.json',
      configPresent: false,
      managedEntriesPresent: false,
      executablePresent: true,
    }
    expect(effectiveConnectorStatusLabel('notInstalled', detected)).toBe(
      'CLI installed — hooks missing',
    )
    expect(
      effectiveConnectorStatusLabel('driftDetected', {
        ...detected,
        source: 'cursor',
        configPresent: true,
      }),
    ).toBe('Hooks need repair')
  })

  it('provides Cursor hooks guidance', () => {
    expect(connectorStatusGuidance('cursor', 'actionNeeded')).toMatch(/cursor settings/i)
  })

  it('provides Codex connect guidance when CLI is installed without hooks', () => {
    expect(
      connectorStatusGuidance('codex', 'notInstalled', [], {
        source: 'codex',
        scope: 'user',
        displayPath: '~/.codex/hooks.json',
        configPresent: false,
        managedEntriesPresent: false,
        executablePresent: true,
      }),
    ).toMatch(/use connect/i)
  })

  it('provides repair guidance when hook config exists without managed entries', () => {
    expect(
      connectorStatusGuidance('cursor', 'driftDetected', [], {
        source: 'cursor',
        scope: 'user',
        displayPath: '~/.cursor/hooks.json',
        configPresent: true,
        managedEntriesPresent: false,
        executablePresent: true,
      }),
    ).toMatch(/repair/i)
  })

  it('maps decision delivery states', () => {
    expect(decisionDeliveryLabel('pending')).toMatch(/pending delivery/i)
    expect(decisionDeliveryLabel('effectObserved')).toMatch(/acknowledged/i)
  })

  it('surfaces process running without inventing a session', () => {
    const entry = {
      source: 'cursor' as const,
      scope: 'user' as const,
      displayPath: '~/.cursor/hooks.json',
      configPresent: false,
      managedEntriesPresent: false,
      executablePresent: false,
      processRunning: true,
      runningProcessName: 'cursor',
    }
    expect(isDetectedConnectorVisible(entry)).toBe(true)
    expect(detectedConnectorSummary(entry)).toMatch(/process running/i)
    expect(detectedConnectorSummary(entry)).toMatch(/session not verified/i)
    expect(connectorStatusGuidance('cursor', 'notFound', [], entry)).toMatch(
      /agent process is running/i,
    )
  })
})
