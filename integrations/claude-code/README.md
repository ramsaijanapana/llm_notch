# Claude Code integration template

**Status:** Observation-only V1. **Not installed automatically.** No claimed approvals.

## Capability honesty (V1)

| Signal | Quality | Notes |
|--------|---------|-------|
| Session lifecycle | Partial | `SessionStart` / `SessionEnd` / `Stop` |
| Tool events | Partial | `PreToolUse` / `PostToolUse` / `PostToolUseFailure` |
| Attention | Partial | `PermissionRequest` is observed for UI attention state only |
| Process attribution | Heuristic | CLI sessions usually have distinct trees; not guaranteed |
| Decision response | **None** | Template never returns `permissionDecision`, `deny`, or exit code `2` |

Claude Code hooks *can* block tool use via `PreToolUse` / `PermissionRequest` responses. **This template does not.** llm_notch V1 surfaces attention in the overlay; it does not auto-approve or auto-deny Claude permissions.

## Install locations

| Scope | File |
|-------|------|
| Project | `.claude/settings.json` |
| User | `~/.claude/settings.json` |
| Local (gitignored) | `.claude/settings.local.json` |

Merge the `hooks` object from [`settings.hooks.template.json`](./settings.hooks.template.json) into your existing settings. Preserve all unrelated keys (`model`, `permissions`, env, other hooks).

## Windows

Replace `sh integrations/wrappers/...` with:

```text
pwsh -NoProfile -File integrations/wrappers/llm-notch-hook-wrapper.ps1 -Source claudeCode -VendorEvent SessionStart
```

(Adjust `-VendorEvent` per hook.)

## Stdin / stdout

- **stdin:** Claude Code hook JSON ([fixtures](../fixtures/claude-code/)).
- **stdout:** `{}` (wrapper fail-open).
- **exit:** `0` always from wrapper.

## Restart required

Hook changes take effect on the next Claude Code session after saving settings.

## References

- [Claude Code hooks guide](https://code.claude.com/docs/en/hooks-guide)
- [Hooks reference](https://code.claude.com/docs/en/hooks)
