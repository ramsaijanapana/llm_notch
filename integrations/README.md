# LLM Notch integration templates

**These files are templates and previews only.** Nothing in this directory is installed automatically. Applying an integration requires an explicit user action through the llm_notch dashboard installer (planned) or manual copy after reviewing a diff.

## Principles

1. **Fail open** — Hook wrappers always exit `0` in vendor hook mode so agent workflows are never blocked by telemetry.
2. **No secrets in hooks** — Authentication uses a per-app-start runtime descriptor read by the signed `llm-notch-hook` helper. Hooks never receive tokens via argv, env, or config.
3. **Preserve unrelated hooks** — Merge templates add llm_notch entries; they do not replace existing hook definitions.
4. **Honest capabilities** — Shipped vendor templates report process attribution as `unknown` because they do not provide a validated PID/start-time pair. Only explicit permission events set attention.
5. **Runtime honesty** — Protocol-v1 host ingest and `llm-notch-hook` transport are implemented. Automatic template installation is not; every config change remains a manual, reviewed action.

## Layout

| Path | Purpose |
|------|---------|
| `wrappers/` | Portable Unix shell and PowerShell hook wrappers |
| `windows-terminal/` | Honest Windows Terminal tab/pane collector docs + limitations |
| `cursor/` | Cursor `hooks.json` template (project scope) |
| `claude-code/` | Claude Code `settings.json` hooks fragment |
| `codex/` | Codex lifecycle `hooks.json` + legacy `notify` fallback |
| `gemini/` | Gemini CLI `settings.json` hooks fragment |
| `qwen/` | Qwen Code `settings.json` hooks fragment |
| `antigravity-cli/` | Antigravity CLI named `.agents/hooks.json` scaffold |
| `copilot/` | GitHub Copilot CLI `hooks/*.json` template |
| `remote/` | SSH remote-host hook templates (`LLM_NOTCH_EVENT_SPOOL=1` spool pairing) |
| `generic/` | `emit` CLI examples for custom agents |
| `fixtures/` | JSON examples for tests and documentation |
| `validate-json.sh` / `validate-json.ps1` | Syntax-check JSON fixtures |

## Documentation

See [`docs/integrations/`](../docs/integrations/README.md) for the protocol guide, capability matrix, security notes, installation diff/backup flow, troubleshooting, and helper install paths.

## Quick validation

```bash
./integrations/validate-json.sh
```

```powershell
.\integrations\validate-json.ps1
```
Bundled first-party connectors currently cover Cursor, Claude Code, Codex, Gemini CLI, Qwen Code, and Copilot CLI. The
dashboard detects their user/project configuration paths, previews merged changes, preserves
unrelated hooks, and creates rollback backups before writing. Antigravity CLI ships connector
merge fixtures and hook stdin mapping tests.
