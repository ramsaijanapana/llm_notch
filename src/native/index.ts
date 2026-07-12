export type { NativeClient } from './client.ts'
export { createNativeClient } from './client.ts'
export { NATIVE_COMMANDS, NATIVE_EVENTS } from './commands.ts'
export { PROTOCOL_VERSION } from './contracts.ts'
export {
  isBrowserMarketingApp,
  isDashboardWindow,
  isOverlayWindow,
  isTauriEnvironment,
} from './environment.ts'
export { isNativeClientError, NativeClientError } from './errors.ts'
export { createFakeNativeClient, FakeNativeClient } from './FakeNativeClient.ts'
export { createPreviewSnapshot, DEFAULT_PUBLIC_SETTINGS } from './fixtures.ts'
export {
  coalesceStreamFrames,
  evaluateStreamSequence,
  mergeMetricsFrame,
} from './streamProcessor.ts'
export { createTauriNativeClient, TauriNativeClient } from './TauriNativeClient.ts'
export type {
  AgentCatalogEntry,
  BootstrapResult,
  CreateNativeClientOptions,
  IntegrationHealthEntry,
  IntegrationHealthReport,
  NativeClientMode,
  OverlayMode,
  StreamSubscription,
} from './types.ts'
