# Generic ingest protocol (v1) for third-party agents

This guide complements [`protocol.md`](./protocol.md) with SDK-oriented examples for custom agents that are not Cursor, Claude Code, or Codex.

## Version

| Field | Value |
|-------|-------|
| `GENERIC_PROTOCOL_VERSION` | `1` |
| Rust module | `notch-adapters-generic` |
| Example fixtures | `integrations/fixtures/protocol/` |

## Onboarding model

1. **No automatic install** — copy CLI examples or embed the ingest wire client in your agent.
2. **Explicit emit** — use `llm-notch-hook emit` with `--source generic` (strict mode; invalid input fails).
3. **Optional ACK** — clients that wait for host persistence may rely on ingest ACK frames documented in `protocol.md`; declare this with `GenericClientCapabilities.supports_response_ack`.

## Documented events (v1)

| `event` | Purpose |
|---------|---------|
| `sessionStart` | Create or upsert a session |
| `sessionEnd` / `update` | End or refresh session state |
| `tool` | Append redacted tool activity |
| `attention` | Observation-only attention latch |
| `lifecycle` | Generic lifecycle marker |
| `remove` | Remove a local session |

## Example: session + tool + attention

See [`integrations/generic/emit-examples.sh`](../../integrations/generic/emit-examples.sh).

```bash
llm-notch-hook emit \
  --source generic \
  --event sessionStart \
  --external-session-id my-agent-1 \
  --label "My agent" \
  --status running

llm-notch-hook emit \
  --source generic \
  --event tool \
  --external-session-id my-agent-1 \
  --summary "Step finished" \
  --tool-name build

llm-notch-hook emit \
  --source generic \
  --event attention \
  --external-session-id my-agent-1 \
  --attention question \
  --summary "Waiting for operator"
```

## Process attribution (optional)

Provide **both** `--pid` and `--process-started-at-ms` from the same live process identity to reach `exact` attribution while the pair validates:

```bash
llm-notch-hook emit \
  --source generic \
  --event sessionStart \
  --external-session-id my-agent-1 \
  --label "My agent" \
  --status running \
  --pid 4242 \
  --process-started-at-ms 1700000000000
```

## Validation

Rust tests validate shipped examples and fixtures:

```bash
cargo test -p notch-adapters-generic
cargo test -p notch-adapters-codex
```

## Privacy boundaries

Same as all llm_notch ingest: no prompt text, command bodies, tool output, or cloud egress crosses the helper boundary.
