/**
 * Tauri command names consumed by the renderer native client seam.
 * Integration owner wires these on the Rust side.
 */
export const NATIVE_EVENTS = {
  remoteConnectionChanged: 'remote-connection-changed',
} as const

export type NativeEventName = (typeof NATIVE_EVENTS)[keyof typeof NATIVE_EVENTS]

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
  listAgentCatalog: 'list_agent_catalog',
  listQuotaSnapshots: 'list_quota_snapshots',
  listRemoteHosts: 'list_remote_hosts',
  upsertRemoteHost: 'upsert_remote_host',
  removeRemoteHost: 'remove_remote_host',
  getRemoteBackendStatus: 'get_remote_backend_status',
  previewRemoteDeploy: 'preview_remote_deploy',
  executeRemoteDeploy: 'execute_remote_deploy',
  startRemoteRelay: 'start_remote_relay',
  stopRemoteRelay: 'stop_remote_relay',
  getRemoteConnectionStatus: 'get_remote_connection_status',
  getSoundThemes: 'get_sound_themes',
  previewSoundRouting: 'preview_sound_routing',
  playSoundEvent: 'play_sound_event',
  importSoundPack: 'import_sound_pack',
  previewConnector: 'preview_connector_change',
  detectConnectors: 'detect_connectors',
  applyConnector: 'apply_connector_change',
  removeConnector: 'remove_connector',
  repairConnector: 'repair_connector',
  rollbackConnector: 'rollback_connector',
  connectorHealth: 'connector_health',
  listConnectorBackups: 'list_connector_backups',
  getPendingDecisions: 'list_pending_decisions',
  submitDecision: 'submit_decision',
} as const

export type NativeCommandName = (typeof NATIVE_COMMANDS)[keyof typeof NATIVE_COMMANDS]
