# Claude Code integration template

**Status:** Observation-first V1 with capability-gated decision responses on known Claude Code versions.

## Capability honesty

| Signal | Quality | Notes |
|--------|---------|-------|
| Session lifecycle | Partial | `SessionStart` / `SessionEnd` / `Stop` |
| Tool events | Partial | `PreToolUse` / `PostToolUse` / `PostToolUseFailure` |
| Attention | Partial | `PermissionRequest` and `ExitPlanMode` (`PreToolUse`) surface attention |
| Process attribution | Unknown | Shipped hooks do not provide validated PID/start-time pairs |
| Decision response | Capability-gated | Known Claude Code ≥ 2.1.0: `PermissionRequest` allow/deny and `ExitPlanMode` approve via documented `PreToolUse` output. Unknown versions: observation-only |
| Question response | **None** | No verified generic `AskUserQuestion` answer path in the shipped template |

Claude Code hooks *can* block tool use and answer permission dialogs. **The shipped template always fails open** (`{}` stdout, exit `0`) until the decision broker delivers a verified response for a supported hook on a known version. llm_notch never simulates vendor success without evidence.

## Install locations

| Scope | File |
|-------|------|
| Project | `.claude/settings.json` |
| User | `~/.claude/settings.json` |
| Local (gitignored) | `.claude/settings.local.json` |

Merge **only** the `hooks` object from [`settings.hooks.template.json`](./settings.hooks.template.json) into your existing settings. Preserve all unrelated keys (`model`, `permissions`, env, other hooks).

The connector substitutes `{{LLM_NOTCH_HELPER}}` with the bundled helper absolute path at apply time.

## Windows

Replace the hook command with the PowerShell wrapper form documented in [helper-paths.md](../../docs/integrations/helper-paths.md), or substitute an absolute `llm-notch-hook.exe` path:

```text
"C:\Program Files\llm_notch\llm-notch-hook.exe" hook --source claudeCode --vendor-event SessionStart --hook-mode
```

## Stdin / stdout

- **stdin:** Claude Code hook JSON ([fixtures](../fixtures/claude-code/)).
- **stdout:** `{}` fail-open neutral output unless the broker returns a verified decision response.
- **exit:** `0` from the helper/wrapper in vendor hook mode.

## Restart required

Hook changes take effect on the next Claude Code session after saving settings.

## Health

- **Installation:** managed llm_notch hook entries present in `settings.json` `hooks`
- **Trust:** not required (no external review step documented for Claude Code hooks)
- **Unknown Claude Code version:** downgrade to observation-only capabilities

## References

- [Claude Code hooks guide](https://code.claude.com/docs/en/hooks-guide)
- [Hooks reference](https://code.claude.com/docs/en/hooks)
- Adapter crate: `crates/notch-adapters/claude-code`
