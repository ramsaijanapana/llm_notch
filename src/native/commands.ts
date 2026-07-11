/**
 * Tauri command names consumed by the renderer native client seam.
 * Integration owner wires these on the Rust side.
 */
export const NATIVE_COMMANDS = {
  bootstrap: 'bootstrap',
  subscribeStream: 'subscribe_stream',
  unsubscribeStream: 'unsubscribe_stream',
  openDashboard: 'open_dashboard',
  openSession: 'open_session',
  setOverlayMode: 'set_overlay_mode',
  acknowledgeAttention: 'acknowledge_attention',
  updateSettings: 'update_settings',
  getHistory: 'get_history',
  getSessionEvents: 'get_session_events',
  listDisplays: 'list_displays',
  purgeHistory: 'purge_history',
  integrationHealth: 'integration_health',
  previewConnector: 'preview_connector_change',
} as const

export type NativeCommandName = (typeof NATIVE_COMMANDS)[keyof typeof NATIVE_COMMANDS]
