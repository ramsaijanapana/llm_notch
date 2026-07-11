# llm_notch integrations documentation

Templates and protocol reference for connecting local AI coding agents to the llm_notch desktop host.

## Index

| Document | Description |
|----------|-------------|
| [protocol.md](./protocol.md) | Generic ingest protocol, sample events, shell examples |
| [capability-matrix.md](./capability-matrix.md) | Honest per-vendor capability comparison |
| [security-privacy.md](./security-privacy.md) | Auth, redaction, and raw-payload handling |
| [installation.md](./installation.md) | Diff preview, backup, rollback, merge rules |
| [helper-paths.md](./helper-paths.md) | macOS and Windows helper binary locations |
| [troubleshooting.md](./troubleshooting.md) | Common failure modes and diagnostics |
| [examples/generated-diff.md](./examples/generated-diff.md) | Example installer diff output |
| [examples/backup-rollback.md](./examples/backup-rollback.md) | Backup naming and rollback procedure |

## Current status

| Component | Status |
|-----------|--------|
| `notch-protocol` wire types | Frozen v1 contracts in `crates/notch-protocol` |
| `llm-notch-hook` helper | Implemented fail-open vendor mapping and authenticated delivery |
| `notch-ipc` transport | Implemented local socket/named pipe, auth, bounds, and spool |
| Dashboard installer | Read-only preview; apply/remove intentionally unavailable |
| Live overlay sessions | Implemented when the desktop host and reviewed hooks are running |

Templates describe the implemented observation-only V1 path. They are not installed automatically; review and apply them manually.

## Template sources

| Vendor | Template path |
|--------|---------------|
| Cursor | [`integrations/cursor/hooks.json.template`](../../integrations/cursor/hooks.json.template) |
| Claude Code | [`integrations/claude-code/settings.hooks.template.json`](../../integrations/claude-code/settings.hooks.template.json) |
| Codex | [`integrations/codex/hooks.json.template`](../../integrations/codex/hooks.json.template) |
| Generic CLI | [`integrations/generic/emit-examples.sh`](../../integrations/generic/emit-examples.sh) |

## Validation

```bash
chmod +x integrations/wrappers/llm-notch-hook-wrapper.sh integrations/validate-json.sh
./integrations/validate-json.sh
```
