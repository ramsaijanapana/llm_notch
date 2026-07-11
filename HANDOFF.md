# Lane 8 — Observability HANDOFF

**Branch:** `feat/lane-8-observability`  
**Base:** `25d056a`  
**Owner:** Lane 8 (Observability)

## Summary

Lane 8 delivers observability parity: honest attribution labels, timestamp-aware history, non-focus-stealing resource alerts (beacon/tray/optional sound), scoped privacy purge with `includeBackups` default false, IPC traffic probes for connector health, and metrics/alert unit tests.

## Delivered

### Metrics & attribution
- `notch-metrics`: aggregate attribution honesty test; missing-root → `unknown` attribution on wire
- UI maps `unknown` → **"Not attributed"** via `attributionQualityLabel` (dashboard + overlay)
- `QualityBadge` and `MetricsPanel` use display labels, not raw wire enums

### History (15m / 1h / 24h)
- Live 15m window rolls `requestedStartMs`/`requestedEndMs` on each sample (timestamp-accurate domain)
- Persisted 1h/24h unchanged: bucket timestamps, coverage labels, independent per-series downsampling

### Resource alerts (no focus steal)
- `AlertEvaluator` resource kinds surfaced on `AppSnapshot.resourceAlerts`
- `AlertNotifier` service: tray tooltip + optional `MessageBeep`/`afplay` sound (off by default via `alertSoundEnabled`)
- Overlay `HealthBeacon` degrades when resource alerts active (attention still takes priority)
- Alerts never call window focus APIs

### Privacy purge
- `PurgeScope` wired end-to-end: history, sessionEvents, connectorJournal, `includeBackups` (default **false**)
- Settings panel scope checkboxes + confirmation copy
- `purge_history` command accepts optional scope; returns `PurgeResult`
- Connector journal purge clears apply log; backups kept unless explicit opt-in

### Connector health traffic
- IPC ingest calls `ConnectorManager::record_event` on session upsert and session events
- Traffic probe uses `lastEventObserved` for `waitingFirstEvent` → `connected` transitions

### Protocol extensions (Lane 8 owned)
- `PublicSettings.alertSoundEnabled` (default false)
- `ResourceAlert` / `ResourceAlertKind` on `AppSnapshot.resourceAlerts`

## Tests run

| Suite | Result |
|-------|--------|
| `cargo test -p notch-protocol --lib` | pass |
| `cargo test -p notch-metrics --lib` | pass (13 + 1 ignored smoke) |
| `cargo test -p notch-core --lib` | pass (36) |
| `cargo test -p notch-connectors --lib` | pass (21) |
| `cargo test -p llm-notch-desktop --lib` | **blocked** — build requires `binaries/llm-notch-hook-x86_64-pc-windows-msvc.exe` |
| Frontend vitest (contracts, overlay helpers/selectors) | run after `npm install` |

## Blockers / follow-ups

1. **Tauri desktop test/build** — missing bundled hook binary in this worktree; observability Rust in `src-tauri` compiles once hook artifact exists.
2. **Backup file deletion on disk** — `includeBackups` clears journal backup entries; physical backup file removal deferred (journal metadata only).
3. **Usage/quota** — intentionally omitted per contract freeze.

## Touch map

| Area | Files |
|------|-------|
| Protocol | `crates/notch-protocol/src/types.rs`, `bindings/*` |
| Core alerts/purge | `crates/notch-core/src/alerts.rs`, `app_core.rs`, `persistence/sqlite.rs` |
| Metrics | `crates/notch-metrics/src/aggregate.rs` |
| Connectors traffic | `crates/notch-connectors/src/manager.rs`, `journal.rs` |
| Tauri host | `src-tauri/src/state.rs`, `services/alerts.rs`, `services/tray.rs`, `commands/*` |
| UI | `SettingsPanel`, `MetricsPanel`, `QualityBadge`, overlay beacon, `NativeSurfaces` |

## Verification commands

```bash
cargo test -p notch-protocol -p notch-metrics -p notch-core -p notch-connectors --lib
npm run test:run -- src/native/contracts.test.ts src/features/native-overlay/model/overlay.helpers.test.ts
```
