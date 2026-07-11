# Lanes 1–4 integration handoff

Consolidated handoff from connector + adapter foundation lanes merged into `feat/integration-foundations`.

---

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

## Known follow-ups (not blockers)

- Wire `record_event` from IPC ingest for live traffic probes
- Adapter-provided template fingerprints for drift vs template (currently managed-entry presence only)
- UI lane: call `repair_connector` / `rollback_connector` + render `ConnectorApplyResult`
- SQLite migration for journal (currently JSON file in app data)

---

# Lane 2 handoff — Cursor adapter

**Branch:** `feat/lane-2-cursor`  
**Crate:** `notch-adapters-cursor` (`crates/notch-adapters/cursor`)  
**Status:** `LANE_2_COMPLETE`

## Summary

Shipped observation-only Cursor hooks adapter: version detection, frozen capability matrix mapping, redacted event normalization, hooks.json merge specs, health probe hints, and updated templates/fixtures aligned with [Cursor hooks docs](https://cursor.com/docs/hooks) (July 2026).

## Connector lane (Lane 1) integration

### Dependencies

- **`notch-adapters-cursor`** — add to `notch-connectors` (or apply engine) when wiring preview/apply.
- **`notch-protocol`** — consume only; no enum changes required.
- **Blocked on:** connector apply engine trait/API not yet merged in this repo. This lane exports stable functions; connector should call them from preview/apply/health paths.

### Placeholder substitution at apply time

| Token | Replace with |
|-------|----------------|
| `{{LLM_NOTCH_HELPER}}` | Absolute path to bundled `llm-notch-hook` |
| `{{LLM_NOTCH_WRAPPER}}` | Optional absolute wrapper path (manual dev / legacy) |

Template command shape:

```text
"{{LLM_NOTCH_HELPER}}" hook --source cursor --vendor-event <event> --hook-mode
```

Use `template_hooks_json(scope, resolved_helper_path)` or `cursor_managed_entries(scope)` + `merge_hooks_json`.

### Merge rules (user + project)

| Scope | Config path | Display path for preview |
|-------|-------------|---------------------------|
| User | `~/.cursor/hooks.json` | `~/.cursor/hooks.json` |
| Project | `.cursor/hooks.json` | `.cursor/hooks.json` |

1. Parse existing JSON (`version: 1` expected).
2. For each event in `MANAGED_EVENTS`, append managed entry if fingerprint absent.
3. Fingerprint: `llm-notch:cursor:<event>` (detect via `is_managed_command`).
4. Preserve all foreign entries.
5. Backup hint: `.cursor/hooks.json.llm-notch.bak.<timestamp>` (project) or `~/.cursor/hooks.json.llm-notch.bak.<timestamp>` (user).

**Managed events (V1):** `sessionStart`, `sessionEnd`, `preToolUse`, `postToolUse`, `postToolUseFailure`, `stop`

### Health probe hints

Call `classify_hooks_commands` on parsed `hooks` object, then `health_probe_hints`:

| Signal | Meaning |
|--------|---------|
| `managed_entry_present` | At least one managed command detected |
| `all_managed_events_present` | All six V1 events installed |
| `helper_path_configured` | No unreplaced `{{LLM_NOTCH_HELPER}}` in managed commands |
| Installation WARN | Partial install or drift (missing events) |
| Helper FAIL | Placeholder not resolved |

Traffic probe remains connector-owned (first successful ingest).

### Version → capabilities

```rust
let profile = detect_version(payload.cursor_version, hooks_json.version);
let caps = capabilities(&profile);
```

- **Known:** semver ≥ 0.45.0, major ≤ 9, hooks schema v1 → `AdapterCapabilities::template(Cursor)`
- **Unknown:** observation-only (events on, no response paths)

## Decision broker lane (Lane 5)

- V1 `decisionResponse: false`, `respond_decisions: false`.
- `hook_response()` always returns `{}`.
- `build_permission_response()` returns `None` unless capabilities explicitly enable `respond_decisions` (future).
- Verified respondable hooks (docs only, not enabled): `preToolUse`, `beforeShellExecution`, `beforeMCPExecution`, `beforeReadFile`, `subagentStart` — allow/deny JSON only. No question/plan/followup responses from llm_notch.

## UI lane (Lane 7)

- Show Cursor as observation-only; hide approve/deny (capability-gated).
- Health copy: "Enable hooks in Cursor Settings → Hooks" (no external trust step).
- Partial install → `driftDetected` via installation probe WARN.

## Hook helper (`notch-hook`)

Optional follow-up: delegate `vendor_hook_payload` for `--source cursor` to `notch_adapters_cursor::normalize_event` → `IngestPayload` to avoid duplicate mapping. Not required for lane 2 completion; normalization logic lives in adapter crate.

## Tests

```bash
cargo test -p notch-adapters-cursor
```

25 tests: unit + vendor/version fixture integration.

## Blockers / open items

| Item | Owner |
|------|-------|
| Connector apply engine calling adapter merge/template APIs | Lane 1 |
| Helper path resolution from Tauri bundle at apply time | Lane 1 + Platform |
| Wire `normalize_event` into `llm-notch-hook` cursor path | Lane 2 optional / Platform |
| Windows installed command quoting (`cmd` vs direct exe) | Lane 1 apply renderer |

No protocol freeze changes required.

---

# Lane 3 handoff — Claude Code adapter

**Branch:** `feat/lane-3-claude`  
**Base:** `327bc0e0d4bd632cb28aae374f7e8f9f5d1d404c`  
**Status:** `LANE_3_COMPLETE`

## Package

`crates/notch-adapters/claude-code` (`notch-adapters-claude-code`)

Registered in root `Cargo.toml` workspace members and `[workspace.dependencies]`.

## Public API (for connector / decision / hook lanes)

| Module | Entry points |
|--------|----------------|
| `template` | `HELPER_PATH_PLACEHOLDER` (`{{LLM_NOTCH_HELPER}}`), `WRAPPER_PATH_PLACEHOLDER`, `render_hook_command`, `template_settings_hooks` |
| `merge` | `merge_settings_hooks`, `is_managed_command`, `entry_fingerprint`, `claude_managed_entries`, `MergeScope` |
| `version` | `detect_version`, `ClaudeVersionProfile` |
| `capabilities` | `capabilities(profile) -> AdapterCapabilities` |
| `normalize` | `normalize_event(vendor_event, payload) -> NormalizedClaudeEvent` (redacted) |
| `response` | `hook_response` (fail-open `{}`), `build_permission_response`, `build_exit_plan_approve_response`, `build_decision_response` |
| `health` | `managed_entry_present`, `health_probe_hints`, `ClaudeHealthHints` (`trust_required: false`) |

## Verified vendor response paths (known Claude Code ≥ 2.1.0)

- `PermissionRequest`: `hookSpecificOutput.decision.behavior` allow/deny
- `ExitPlanMode`: `PreToolUse` with `permissionDecision: "allow"` + required `updatedInput` object

**Not implemented:** generic `AskUserQuestion` answer path.

Unknown Claude Code versions → observation-only (`respondDecisions: false`).

## Connector integration notes

1. **Apply merge:** call `merge_settings_hooks(existing_settings, template_settings_hooks())` — preserves `permissions`, `model`, env, and unrelated keys; merges only `hooks`.
2. **Placeholder substitution:** replace `{{LLM_NOTCH_HELPER}}` with bundled absolute helper path before write.
3. **Health installation probe:** use `managed_entry_present` / `health_probe_hints`; trust axis should stay OK (`trust_required: false`).
4. **Fingerprint dedupe:** `(event, matcher, command)` via `entry_fingerprint`.

## Decision broker integration notes

1. Normalize inbound hook payloads with `normalize_event`; use `decision_kind` + `respondable_hook` to gate UI controls.
2. On user action, build stdout JSON via `build_decision_response` only when `capabilities(profile).respond_decisions` is true.
3. Default helper/wrapper stdout remains `hook_response()` (`{}`) — never simulate vendor success without delivery evidence.

## Templates & fixtures

- Template: `integrations/claude-code/settings.hooks.template.json` (synced with Rust template via test)
- Fixtures: `integrations/fixtures/claude-code/*.json` (8 files including permission + ExitPlanMode)
- Docs updated: `integrations/claude-code/README.md`, `docs/integrations/capability-matrix.md` (Claude section)

## Tests

```bash
cargo test -p notch-adapters-claude-code
```

25 tests passing.

## Blockers / dependencies

- **Connector lane:** must wire `notch-adapters-claude-code` into preview/apply/health (API above; apply engine not in this lane).
- **Decision broker:** must consume `NormalizedClaudeEvent` + response builders; not wired in this lane.
- **Hook helper (`notch-hook`):** still uses inline vendor mapping; optional future refactor to call `normalize_event` (out of lane scope).

---

# Lane 4 handoff — Codex + generic adapters

**Branch:** `feat/lane-4-codex`  
**Base:** `327bc0e0d4bd632cb28aae374f7e8f9f5d1d404c`  
**Status:** `LANE_4_COMPLETE`

## Summary

Implemented Codex lifecycle hooks adapter and generic protocol SDK crates, refreshed integration templates/docs per current Codex hooks reference, and added fixture-backed tests. Observation-only V1: wrappers fail open (`{}`), no automated `/hooks` trust.

## New crates

| Crate | Path | Purpose |
|-------|------|---------|
| `notch-adapters-codex` | `crates/notch-adapters/codex/` | Version probing, normalize+redact, merge, templates, external trust hints |
| `notch-adapters-generic` | `crates/notch-adapters/generic/` | Protocol v1 validation, emit examples, optional ACK capability declaration |

Root `Cargo.toml` adds both workspace members and path dependencies.

## Key APIs (for connector/detection lanes)

### Codex

- `notch_adapters_codex::normalize_event(vendor_event, &json)` — redacts sensitive fields, maps lifecycle/tool/stop/PermissionRequest
- `notch_adapters_codex::detect_version(vendor_event, hook_event_name)` — hooks vs notify vs unknown
- `notch_adapters_codex::capabilities(&profile)` — unknown → observation-only
- `notch_adapters_codex::external_trust_actions()` — `CodexHooksReview` instructions for `/hooks` (never automated)
- `notch_adapters_codex::template_hooks_json()` / `merge_hooks_json` — installer merge helpers
- `notch_adapters_codex::build_permission_response` — documented allow/deny shapes (not emitted by shipped template)

### Generic

- `notch_adapters_generic::GENERIC_PROTOCOL_VERSION` (= 1)
- `notch_adapters_generic::validate_ingest_example(&IngestPayload)`
- `notch_adapters_generic::GenericClientCapabilities` + `capabilities_with_ack`

## Integration templates updated

- `integrations/codex/hooks.json.template` — `PermissionRequest`, `startup|resume` matcher, `{{LLM_NOTCH_WRAPPER_ABSOLUTE_PATH}}`, `features.hooks` guidance
- `integrations/codex/config.inline-hooks.example.toml` — inline TOML alternative
- `integrations/codex/config.notify.fallback.toml` — legacy notify with placeholders
- `integrations/codex/README.md` — trust, partial attention, observation-only PermissionRequest
- `integrations/generic/README.md` — onboarding docs only (no install)
- `docs/integrations/generic-protocol.md` — third-party SDK guide

## Tests

```bash
cargo test -p notch-adapters-codex -p notch-adapters-generic
```

**Results:** 23 codex + 10 generic tests passing (2026-07-11).

## Blockers / follow-ups for other lanes

| Item | Owner | Notes |
|------|-------|-------|
| Wire adapters into connector preview/health | Connectors lane | Use `external_trust_actions`, `merge_hooks_json`, `health_probe_hints` |
| Optional: delegate `notch-hook` codex mapping to adapter crate | Hook lane | Not required for lane 4; hook still has inline mapping |
| `AdapterCapabilities::template(Codex)` attention still `None` in protocol | Composer | Adapter crate returns `Partial` for lifecycle hooks; connector should prefer adapter caps |
| Apply/remove still disabled | Connectors | Frozen contract — preview only |
