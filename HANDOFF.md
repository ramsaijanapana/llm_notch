# Lane 4 handoff — Codex + generic adapters

**Branch:** `feat/lane-4-codex`  
**Base:** `327bc0e0d4bd632cb28aae374f7e8f9f5d1d404c`  
**Status:** `LANE_4_COMPLETE`

## Summary

Implemented Codex lifecycle hooks adapter and generic protocol SDK crates, refreshed integration templates/docs per current Codex hooks reference, and added fixture-backed tests. Observation-only V1: wrappers fail open (`{}`), no automated `/hooks` trust.

## Commits

Single commit on `feat/lane-4-codex` (see `git log -1` after pull).

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
- Stale `features.codex_hooks` references fixed in owned docs (canonical `features.hooks`)

## Fixtures added

- `integrations/fixtures/codex/permission-request-input.json`
- `integrations/fixtures/codex/pre-tool-use-input.json`
- `integrations/fixtures/codex/post-tool-use-input.json`
- `integrations/fixtures/codex/user-prompt-submit-input.json`

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

## Out of scope (untouched)

- Connectors apply engine, cursor/claude adapters, protocol enum changes, UI, broker, other worktrees
