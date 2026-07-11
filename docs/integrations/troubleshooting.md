# Troubleshooting integrations

## Quick diagnostics

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| No sessions in overlay | Desktop app not running, helper path wrong, or hooks not firing | Start llm_notch; verify the reviewed hook and helper path |
| Hooks never fire | Config not loaded / wrong path | Check vendor Hooks settings; restart agent |
| `llm-notch-hook not found` | Helper not on PATH | Set `LLM_NOTCH_HOOK_BIN` or use absolute bundle path |
| Integration shows degraded | Capability template loaded but live connector health is not independently verified | Fire a reviewed hook and inspect host logs |
| Codex hooks skipped | Untrusted hook defs | Run `/hooks`, review and trust |
| Cursor blocked by hook | Non-llm_notch hook or `failClosed: true` | llm_notch wrapper always exits 0 — check other hooks |
| Stale sessions | Missing `sessionEnd` | Ensure `sessionEnd` / `Stop` hooks installed |

## Cursor

### Hooks tab empty

1. Confirm `hooks.json` exists at project `.cursor/hooks.json` or `~/.cursor/hooks.json`
2. Validate JSON: `./integrations/validate-json.sh`
3. Restart Cursor

### Wrapper path wrong

Project hooks run with cwd = project root. User hooks run with cwd = `~/.cursor/`.

| Scope | Command path example |
|-------|---------------------|
| Project | `integrations/wrappers/llm-notch-hook-wrapper.sh` |
| User | `hooks/llm-notch-hook-wrapper.sh` |

### Matcher / failClosed

llm_notch template uses no matchers. If you add matchers, test without them first (per Cursor hook skill).

### Permission inference false positives

`preToolUse` is recorded as tool activity and does not set attention. Cursor attention remains unavailable unless a future explicit permission event is added.

## Claude Code

### Hooks not running

- Edit `settings.json` directly; `/hooks` is read-only in Claude Code
- Restart session after changes
- Check `disableAllHooks` is not `true`

### Accidental blocking

If Claude stops after hook install, ensure you did **not** add `permissionDecision` outputs. llm_notch template is observation-only.

### Windows `sh` missing

Use the PowerShell wrapper with `pwsh -NoProfile -File ...`.

## Codex

### Lifecycle hooks disabled

```bash
codex -c features.hooks=true
```

The deprecated alias `features.codex_hooks=true` still works on older builds — prefer `features.hooks`.

### Trust prompt at startup

Codex prints a warning when untrusted hooks exist. Open `/hooks`, trust each llm_notch entry, or disable until reviewed.

### Legacy notify only

If using `config.notify.fallback.toml`:

- Expect completion-only events
- `events: false` in capability matrix
- Plan migration to `hooks.json.template`

## Generic emit

### `emit` fails but hooks work

Hook mode uses the bounded spool when the host is down; explicit `emit` may return non-zero for invalid input or non-discovery delivery errors.

### Process metrics missing

Register `processRoot` with both `pid` and `startedAtMs`:

```bash
llm-notch-hook emit --source generic --event sessionStart \
  --external-session-id generic-1 --label "Generic agent" --status running \
  --pid 4242 --process-started-at-ms 1700000000000
```

## Wrapper debugging

Enable stderr from helper (non-hook mode only):

```bash
llm-notch-hook hook --source cursor --vendor-event sessionStart --hook-mode \
  < integrations/fixtures/cursor/session-start-input.json
```

Hook mode intentionally suppresses errors. Check:

1. `echo '{}' | integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event sessionStart; echo exit:$?`
2. Expect `exit:0` and stdout `{}`

## JSON validation failures

```bash
./integrations/validate-json.sh
```

Fix trailing commas, BOM, or template comments — JSON templates must be valid JSON ( `_comment` keys are allowed).

## Rate limits (host side)

If ingest returns rate errors:

- Sustained: 20 events/s per client
- Burst: 128 per client
- Global: 500/s

Reduce hook fan-out (e.g. drop `preToolUse` matcher `.*` → narrower matcher) — after confirming llm_notch ingest is live.

## Getting help

Collect:

1. llm_notch app version / build channel
2. Vendor + scope (project vs user)
3. Redacted `hooks.json` or settings fragment (no secrets)
4. Result of `./integrations/validate-json.sh`
5. Whether `LLM_NOTCH_HOOK_BIN` is set

Do not paste runtime descriptor contents — they contain auth material.
