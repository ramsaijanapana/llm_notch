# Cursor integration template

**Status:** Observation-only V1. **Not installed automatically.**

## Capability honesty (V1)

| Signal | Quality | Notes |
|--------|---------|-------|
| Session lifecycle | Partial | `sessionStart` / `sessionEnd` / `stop` fire reliably when hooks are enabled |
| Tool events | Partial | `preToolUse` / `postToolUse` / `postToolUseFailure` provide tool name and timing, not full I/O |
| Attention | None | Ordinary `preToolUse` is tool activity and never latches permission attention |
| Process attribution | Unknown | Template does not provide a validated PID/start-time pair |
| Decision response | None | Helper always exits `0` and returns `{}`; llm_notch does not approve/deny Cursor tool calls |

## Placeholder convention

The connector installer replaces template tokens before writing `hooks.json`:

| Token | Meaning |
|-------|---------|
| `{{LLM_NOTCH_HELPER}}` | Absolute path to bundled `llm-notch-hook` binary |
| `{{LLM_NOTCH_WRAPPER}}` | Optional absolute path to `llm-notch-hook-wrapper` when wrapper-based invocation is preferred |

Installed commands invoke the helper directly:

```text
"{{LLM_NOTCH_HELPER}}" hook --source cursor --vendor-event sessionStart --hook-mode
```

Cursor's per-entry `timeout` (2s) bounds execution. The helper fails open in hook mode.

For local development without the dashboard installer, copy [`hooks.json.template`](./hooks.json.template) and substitute an absolute helper path manually, or use the repo wrapper:

```text
sh integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event sessionStart
```

Set `LLM_NOTCH_HOOK_BIN` when the helper is not on `PATH`.

## Install locations

| Scope | Config file | Notes |
|-------|-------------|-------|
| Project | `.cursor/hooks.json` | Commands use absolute helper path after install |
| User | `~/.cursor/hooks.json` | Same command shape; installer resolves helper path |

On Windows, use `llm-notch-hook-wrapper.ps1` for manual dev testing:

```powershell
pwsh -NoProfile -File integrations/wrappers/llm-notch-hook-wrapper.ps1 -Source cursor -VendorEvent sessionStart
```

## Shipped hook events (V1)

Per [Cursor hooks docs](https://cursor.com/docs/hooks):

| Event | Purpose |
|-------|---------|
| `sessionStart` | Session lifecycle |
| `sessionEnd` | Session lifecycle |
| `preToolUse` | Tool activity (observation only) |
| `postToolUse` | Tool activity |
| `postToolUseFailure` | Tool failure signal |
| `stop` | Agent turn completion |

## Stdin / stdout contract

- **stdin:** Cursor hook JSON (see `integrations/fixtures/cursor/`).
- **stdout:** `{}` from the helper in V1 (fail-open, observation-only).
- **exit code:** Always `0` from the helper in hook mode.

`preToolUse` and `beforeShellExecution` *can* return `{ "permission": "allow"|"deny" }` in native Cursor hooks ([docs](https://cursor.com/docs/hooks)), but this template deliberately does not. llm_notch V1 does not broker approvals back into Cursor. Question/plan responses are unsupported.

## Merge behavior

When applied through the dashboard installer (connector lane):

1. Parse current `hooks.json` (`version: 1`).
2. Append llm_notch managed entries only for V1 events (see `notch-adapters-cursor::MANAGED_EVENTS`).
3. Skip any event where a managed entry fingerprint already exists (`llm-notch:cursor:<event>`).
4. Preserve all foreign hook entries unchanged.
5. Write backup to `.cursor/hooks.json.llm-notch.bak.<timestamp>` before replace.

## Managed entry detection (health)

A **managed entry present** probe passes when any command string matches:

- contains `--source cursor`
- contains `--vendor-event <event>`
- contains `--hook-mode` or `llm-notch`

Full install requires all six V1 events above with a resolved helper path (no unreplaced `{{LLM_NOTCH_HELPER}}`).

## Enable hooks in Cursor

1. Open **Cursor Settings → Hooks**.
2. Confirm project or user `hooks.json` is loaded.
3. Restart Cursor or start a new agent session after changes.

## Template file

Copy [`hooks.json.template`](./hooks.json.template) after review. Do not commit live `.cursor/hooks.json` unless your team explicitly wants shared project hooks.

## Rust adapter

Logic lives in `crates/notch-adapters/cursor` (`notch-adapters-cursor`):

- `detect_version` / `capabilities`
- `normalize_event` (redacted protocol mapping)
- `merge_hooks_json` / managed entry fingerprints
- `health_probe_hints`
