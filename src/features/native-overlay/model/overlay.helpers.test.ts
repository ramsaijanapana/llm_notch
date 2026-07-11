import { describe, expect, it } from 'vitest'
import {
  formatAgentSource,
  formatAttentionKind,
  formatAttributionQuality,
  formatBytes,
  formatCpuPercent,
  formatIoQuality,
  formatMetricAvailability,
  formatRuntime,
  formatSessionStatus,
  formatThroughput,
  summarizeSparkline,
} from './overlay.helpers'

describe('overlay.helpers', () => {
  it('formats agent sources and session statuses', () => {
    expect(formatAgentSource('claudeCode')).toBe('Claude Code')
    expect(formatSessionStatus('waiting')).toBe('Waiting')
    expect(formatAttentionKind('permission')).toBe('Permission needed')
  })

  it('maps quality labels including unavailable attribution', () => {
    expect(formatAttributionQuality('exact')).toBe('Exact')
    expect(formatAttributionQuality('shared')).toBe('Shared')
    expect(formatAttributionQuality('heuristic')).toBe('Heuristic')
    expect(formatAttributionQuality('unknown')).toBe('Not attributed')
  })

  it('labels Windows all I/O quality as All I/O', () => {
    expect(formatIoQuality('allIo')).toBe('All I/O')
    expect(formatIoQuality('disk')).toBe('Disk')
    expect(formatIoQuality('partial')).toBe('Partial')
    expect(formatIoQuality('unavailable')).toBe('Unavailable')
  })

  it('formats metric availability states', () => {
    expect(formatMetricAvailability('available')).toBe('Available')
    expect(formatMetricAvailability('warmingUp')).toBe('Warming up')
    expect(formatMetricAvailability('unavailable')).toBe('Unavailable')
  })

  it('formats CPU with warming and unavailable states', () => {
    expect(formatCpuPercent(42, 'available')).toBe('42%')
    expect(formatCpuPercent(undefined, 'warmingUp')).toBe('…')
    expect(formatCpuPercent(undefined, 'unavailable')).toBe('—')
  })

  it('formats bytes, throughput, and runtime', () => {
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatBytes(2 * 1024 ** 2)).toBe('2.0 MB')
    expect(formatThroughput(2048)).toBe('2.0 KB/s')
    expect(formatRuntime(125_000)).toBe('2m 5s')
    expect(formatRuntime(3_720_000)).toBe('1h 2m')
  })

  it('summarizes sparkline ranges for assistive labels', () => {
    expect(summarizeSparkline([])).toMatch(/No CPU samples/)
    expect(summarizeSparkline([10, 20, 30])).toMatch(/Latest 30 percent/)
  })
})
