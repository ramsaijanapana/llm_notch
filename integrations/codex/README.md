# Codex integration template

**Status:** Stable / enabled by default in current Codex builds. **Requires explicit trust in `/hooks`.** Not installed automatically.

## Before you enable

1. Confirm your Codex build supports lifecycle hooks (`features.hooks`; the legacy `features.codex_hooks` flag is deprecated).
2. Open `/hooks` in the Codex CLI and **review + trust** each llm_notch hook definition.
3. Untrusted hooks are skipped — llm_notch will show the integration as `actionNeeded` until trust is complete.

## Capability honesty (V1)

| Signal | Quality | Notes |
|--------|---------|-------|
| Session lifecycle | Partial | `SessionStart`, `Stop` when hooks enabled |
| Tool events | Partial | `PreToolUse` / `PostToolUse`; not all tool paths are hook-interceptable per Codex docs |
| Attention | None | No reliable permission/approval channel in V1 templates |
| Process attribution | Heuristic | Distinct CLI trees expected; not verified at install time |
| Decision response | None | Wrapper never returns `block` / `continue` decisions |

## Preferred: lifecycle hooks

Copy [`hooks.json.template`](./hooks.json.template) to your Codex config directory (commonly `~/.codex/hooks.json` or project `.codex/hooks.json`). Adjust wrapper paths to absolute locations after copying `integrations/wrappers/` out of the repo.

## Fallback: legacy `notify`

Use [`config.notify.fallback.toml`](./config.notify.fallback.toml) **only** if lifecycle hooks are unavailable. This fires after turn completion only — strictly weaker than lifecycle hooks. Codex is deprecating `notify`.

## Stdin / stdout

- **stdin:** Codex hook JSON ([fixtures](../fixtures/codex/)).
- **stdout:** `{}` (fail-open).
- **exit:** `0` from wrapper.

## References

- [Codex hooks documentation](https://developers.openai.com/codex/hooks)
