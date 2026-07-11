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

## Out of scope (untouched)

- `notch-connectors`, cursor/codex/generic adapters, protocol freeze, UI, broker implementation, other worktrees
