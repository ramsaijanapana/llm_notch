# Lane 10 parity RC — merged lanes 5–9

**Branch:** `feat/parity-rc`  
**Base:** `feat/integration-foundations` @ `25d056a`

This branch merges decision (5), context (6), UI (7), observability (8), and platform (9).

See [docs/parity/RC_STATUS.md](docs/parity/RC_STATUS.md) for the release-candidate parity matrix, test evidence, and remaining blockers.

## IPC surface (canonical)

| Command | Args | Returns |
|---------|------|---------|
| `list_pending_decisions` | — | `DecisionRequest[]` |
| `submit_decision` | `request_id`, `DecisionResponse` | `DecisionResponseRecord` |
| `open_session` | `session_id` | `{ contextOpenTier, activated, message? }` |
| `list_connector_backups` | — | `BackupJournalEntry[]` |

Renderer method names (`getPendingDecisions`, `respondDecision`) map to the Lane 5 Rust command names above.

## Context-open tiers (honest)

Cursor, Claude Code, and Codex advertise `contextOpenTier: appActivate` when process identity is available. Terminal hosts may achieve `windowFocus` or `exactPane` when verified.

## Platform notes (lane 9)

- Windows overlay: topmost, non-activating tool window
- macOS: AppKit panel emulation (not true NSPanel)
- Helper sidecar resolved via `runtime/helper_path.rs`

## Observability notes (lane 8)

- Resource alerts via tray/beacon (no focus steal)
- Scoped purge with `includeBackups` default false
- IPC ingest calls `ConnectorManager::record_event` for traffic probes
