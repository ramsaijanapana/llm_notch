# Lane 1 — Connector manager HANDOFF

**Branch:** `feat/lane-1-connectors`  
**Worktree:** `llm_notch-wt-connectors`

## Commits

- `c786eec` — `feat(connectors): implement connector manager with preview, apply, health, rollback`

## Public API — `notch-connectors`

| Type / fn | Purpose |
|-----------|---------|
| `ConnectorConfig` | `repo_root`, `app_data_dir`, `helper_path`, `workspace_root`, optional `user_scope_root` (tests) |
| `ConnectorManager` | Main planner/apply/health surface |
| `ConnectorManager::detect_all` / `detect_source` | Allowlisted-path detection |
| `ConnectorManager::preview_install` / `preview_remove` / `preview_repair` / `preview_rollback` | Short-lived plans (5 min TTL) |
| `ConnectorManager::apply(plan_id)` | Apply preview; accepts plan ID only |
| `ConnectorManager::remove(source, scope)` | Preview+apply removal shortcut |
| `ConnectorManager::health_report` / `connector_health` | Orthogonal probes → `ConnectorUserStatus` |
| `ConnectorManager::record_event` | Traffic probe input from host |
| `DetectedConnector` | Detection DTO (serializable for Tauri) |
| `AdapterDescriptor` / `AdapterRegistry` | Per-vendor template + target metadata |
| `PlanOperation` | `Install`, `Remove`, `Repair`, `Rollback` |

## Tauri commands wired (`integration.rs`)

- `detect_connectors`
- `preview_connector_change(source, scope?)`
- `apply_connector_change(plan_id)` → `ConnectorApplyResult`
- `remove_connector(source, scope?)` → `ConnectorApplyResult`
- `repair_connector(source, scope?)` → `ConnectorPlanPreview`
- `rollback_connector(backup_id)` → `ConnectorPlanPreview`
- `integration_health` / `connector_health` — real probes (not template stubs)

## Cargo.toml changes

Root workspace `Cargo.toml`:

```toml
members = [ ..., "crates/notch-connectors" ]

[workspace.dependencies]
notch-connectors = { path = "crates/notch-connectors" }
```

`src-tauri/Cargo.toml`:

```toml
notch-connectors.workspace = true
```

No other manifest edits.

## Helper binary (local dev)

Tauri build expects:

```
src-tauri/binaries/llm-notch-hook-x86_64-pc-windows-msvc.exe
```

Build and copy:

```powershell
cargo build -p notch-hook
Copy-Item target\debug\llm-notch-hook.exe `
  src-tauri\binaries\llm-notch-hook-x86_64-pc-windows-msvc.exe
```

Override at runtime with `LLM_NOTCH_HOOK_BIN`.

## What adapter lanes (2–4) must provide

| Item | Owner | Notes |
|------|-------|-------|
| Vendor template JSON | Adapter lane | Path under `integrations/{cursor,claude-code,codex}/` |
| **Template fingerprint** | Adapter | SHA-256 of normalized template (no `_comment`, helper path materialized) for drift detection v2 |
| **Managed entry marker** | Connector (frozen) | Commands containing `llm-notch-hook` |
| **Managed entry IDs** | Adapter docs | Stable `(event[, matcher], command)` triples after materialization |
| `external_trust_actions` | Adapter | e.g. Codex `/hooks` review copy |
| Capability truth | Adapter | `AdapterCapabilities` from host registry, not connector crate |

Connector manager currently reads **existing** vendor templates from `integrations/` as interim defaults until adapter lanes register richer descriptors.

## Tests

```bash
cargo test -p notch-connectors   # 25 tests (unit + integration)
cargo check -p notch-connectors
cargo check -p llm-notch-desktop   # requires helper binary in src-tauri/binaries/
```

Coverage: merge/idempotency/preservation, backup/atomic/rollback, concurrency/lock, path traversal rejection.

## Known follow-ups (not blockers)

- Wire `record_event` from IPC ingest for live traffic probes
- Adapter-provided template fingerprints for drift vs template (currently managed-entry presence only)
- UI lane: call `repair_connector` / `rollback_connector` + render `ConnectorApplyResult`
- SQLite migration for journal (currently JSON file in app data)
