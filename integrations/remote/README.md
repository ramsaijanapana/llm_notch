# Remote SSH host hook templates

**Templates for agents running on a remote SSH host** monitored by the desktop relay. These differ from the local integration templates under `cursor/`, `claude-code/`, and `codex/`: remote hooks must spool events into the relay runtime directory instead of the desktop host IPC socket.

## Runtime directory pairing

| Component | Setting |
|-----------|---------|
| Desktop relay start (`start_relay`) | Passes `--event-spool ~/.llm-notch` to the remote `llm-notch-relay` process |
| Remote `llm-notch-hook` | `LLM_NOTCH_EVENT_SPOOL=1` (resolves to `~/.llm-notch`) or `--spool-dir ~/.llm-notch` |
| Spool frames | Written to `~/.llm-notch/spool/*.frame` and watched by the relay |

`LLM_NOTCH_EVENT_SPOOL=1` is truthy shorthand for the default private runtime dir (`~/.llm-notch`). Use an explicit path only when you intentionally override the relay spool directory.

## Local IPC is unchanged

Do **not** set `LLM_NOTCH_EVENT_SPOOL` on hooks that run on the desktop machine. Local vendor templates under `integrations/cursor/` and siblings keep default IPC delivery to the running llm_notch host.

## Install steps (manual, reviewed)

1. Deploy the relay binary from the dashboard **Remote** tab (`execute deploy`).
2. Install `llm-notch-hook` on the remote host (same architecture as the agent).
3. Copy or adapt a template below into the agent's hook config on the **remote** host.
4. Start the relay from the dashboard; confirm connection state reaches **Streaming**.
5. Trigger a vendor session event on the remote host and verify the desktop ingests a `SessionEvent`.

## Templates

| Agent | Template |
|-------|----------|
| Cursor (project hooks) | [`hooks.cursor.template.json`](./hooks.cursor.template.json) |

Other vendors: prefix the managed command from the local template with `LLM_NOTCH_EVENT_SPOOL=1` (or add the env var to the wrapper invocation). Example for Claude Code:

```json
"command": "LLM_NOTCH_EVENT_SPOOL=1 \"{{LLM_NOTCH_HELPER}}\" hook --source claudeCode --vendor-event SessionStart --hook-mode"
```

## Wrapper note

`integrations/wrappers/llm-notch-hook-wrapper.sh` forwards the child process environment. Export `LLM_NOTCH_EVENT_SPOOL=1` in the remote agent shell profile or prefix the wrapper command when using Codex-style `sh wrapper.sh` invocations.
