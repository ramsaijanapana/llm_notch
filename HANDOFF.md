# Lane 10 parity RC — merged lanes 5–9

**Branch:** `feat/parity-rc`  
**Base:** `feat/integration-foundations` @ `25d056a`

This branch merges decision (5), context (6), UI (7), observability (8), and platform (9).

See [docs/parity/RC_STATUS.md](docs/parity/RC_STATUS.md) for the release-candidate parity matrix, test evidence, and remaining blockers.

## Lane summaries (pre-merge)

| Lane | Branch | Focus |
|------|--------|-------|
| 5 | `feat/lane-5-decision` | Decision broker, hook IPC, `list_pending_decisions` / `submit_decision` |
| 6 | `feat/lane-6-context` | Context locators, `open_session` → `OpenSessionResult` |
| 7 | `feat/lane-7-ui` | Onboarding, integrations panel, decision surface |
| 8 | `feat/lane-8-observability` | Alerts, purge scopes, traffic probes, `record_event` |
| 9 | `feat/lane-9-platform` | Overlay platform, CI, release gates |

## IPC surface (canonical)

Decision commands use Lane 5 names:

- `list_pending_decisions` → `DecisionRequest[]`
- `submit_decision` (`request_id`, `DecisionResponse`) → `DecisionResponseRecord`

Context:

- `open_session` → `{ contextOpenTier, activated, message? }`

Integration:

- `list_connector_backups` (wired in platform/connectors lane)

## Lane 8 observability notes

- Resource alerts via tray/beacon (no focus steal)
- Scoped purge with `includeBackups` default false
- IPC ingest calls `ConnectorManager::record_event` for traffic probes
- `PublicSettings.alertSoundEnabled` (default false)
