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

## Files touched

```
Cargo.toml
crates/notch-adapters/cursor/**          (new crate)
integrations/cursor/hooks.json.template
integrations/cursor/README.md
integrations/fixtures/cursor/**          (updated + new)
HANDOFF.md
```

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
