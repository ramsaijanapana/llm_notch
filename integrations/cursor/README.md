# Cursor integration template

**Status:** Observation-only V1. **Not installed automatically.**

## Capability honesty (V1)

| Signal | Quality | Notes |
|--------|---------|-------|
| Session lifecycle | Partial | `sessionStart` / `sessionEnd` / `stop` fire reliably when hooks are enabled |
| Tool events | Partial | `preToolUse` / `postToolUse` / `postToolUseFailure` provide tool name and timing, not full I/O |
| Attention | None | Ordinary `preToolUse` is tool activity and never latches permission attention |
| Process attribution | Shared | Cursor may pool multiple sessions in one process tree; metrics are shared, not per-session exact |
| Decision response | None | Wrapper always exits `0` and returns `{}`; llm_notch does not approve/deny Cursor tool calls |

## Install locations

| Scope | Config file | Wrapper path style |
|-------|-------------|-------------------|
| Project | `.cursor/hooks.json` | `integrations/wrappers/llm-notch-hook-wrapper.sh` (from repo root) |
| User | `~/.cursor/hooks.json` | `hooks/llm-notch-hook-wrapper.sh` (copy wrapper into `~/.cursor/hooks/`) |

On Windows, use `llm-notch-hook-wrapper.ps1` with `pwsh -NoProfile -File ...`.

## Stdin / stdout contract

- **stdin:** Cursor hook JSON (see `fixtures/cursor/`).
- **stdout:** `{}` from the wrapper (fail-open, observation-only).
- **exit code:** Always `0` from the wrapper. Non-zero from the helper is ignored in hook mode.

`preToolUse` *can* return `permission` in native Cursor hooks, but this template deliberately does not. It is recorded as redacted tool activity only. llm_notch V1 does not broker approvals back into Cursor.

## Merge behavior

When applied through the dashboard installer (planned), existing unrelated hook entries must be preserved. Example merge rule:

1. Parse current `hooks.json`.
2. Append llm_notch commands only for events listed in the template.
3. Skip any event where an identical `command` string already exists.
4. Write backup to `.cursor/hooks.json.llm-notch.bak.<timestamp>` before replace.

## Enable hooks in Cursor

1. Open **Cursor Settings → Hooks**.
2. Confirm project or user `hooks.json` is loaded.
3. Restart Cursor or start a new agent session after changes.

## Template file

Copy [`hooks.json.template`](./hooks.json.template) after review. Do not commit live `.cursor/hooks.json` unless your team explicitly wants shared project hooks.
