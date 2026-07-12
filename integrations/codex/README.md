# Codex integration template

**Status:** Stable / enabled by default in current Codex builds. **Requires explicit trust in `/hooks`.** Not installed automatically.

## Before you enable

1. Confirm your Codex build supports lifecycle hooks (`features.hooks`; the legacy `features.codex_hooks` flag is deprecated).
2. Copy [`hooks.json.template`](./hooks.json.template) to your Codex config directory (commonly `~/.codex/hooks.json` or project `.codex/hooks.json`).
3. Replace `{{LLM_NOTCH_HELPER}}` with the absolute path to the bundled `llm-notch-hook` binary (the connector installer does this automatically).
4. Open `/hooks` in the Codex CLI and **review + trust** each llm_notch hook definition. llm_notch never automates this step.
5. Untrusted hooks are skipped — llm_notch shows the integration as `actionNeeded` until trust is complete.

## Capability honesty (V1)

| Signal | Quality | Notes |
|--------|---------|-------|
| Session lifecycle | Partial | `SessionStart`, `Stop` when hooks enabled and trusted |
| Tool events | Partial | `PreToolUse` / `PostToolUse`; not all tool paths are hook-interceptable per Codex docs |
| Attention | Partial | `PermissionRequest` is observed; template never returns allow/deny decisions |
| Process attribution | Heuristic | Distinct CLI trees expected; not verified at install time |
| Decision response | None | Wrapper always returns `{}` (fail-open) |

## Preferred: lifecycle hooks

Copy [`hooks.json.template`](./hooks.json.template) or render programmatically via `notch-adapters-codex::template_hooks_json`. Hook commands invoke the bundled helper directly (`"{{LLM_NOTCH_HELPER}}" hook --source codex ...`).

Equivalent inline TOML lives in [`config.inline-hooks.example.toml`](./config.inline-hooks.example.toml). Prefer **one** representation per config layer (`hooks.json` **or** inline `[hooks]`, not both).

Enable hooks:

```bash
codex -c features.hooks=true
```

## Fallback: legacy `notify`

Use [`config.notify.fallback.toml`](./config.notify.fallback.toml) **only** if lifecycle hooks are unavailable. This fires after turn completion only — strictly weaker than lifecycle hooks. Codex is deprecating `notify`.

## PermissionRequest (observation-only)

Codex documents allow/deny responses for `PermissionRequest`. llm_notch V1 observes the event and maps it to local attention state only. The wrapper stdout remains `{}` so the normal Codex approval flow continues unchanged.

## Stdin / stdout

- **stdin:** Codex hook JSON ([fixtures](../fixtures/codex/)).
- **stdout:** `{}` (fail-open).
- **exit:** `0` from wrapper.

## External trust

After install, complete the guided step surfaced as `externalTrustActions`:

> Open the Codex CLI, run `/hooks`, review each llm_notch hook definition, and trust it.

## References

- [Codex hooks documentation](https://developers.openai.com/codex/hooks)
- Rust adapter: `crates/notch-adapters/codex`
