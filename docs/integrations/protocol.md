# Local ingest protocol (v1)

This document describes the implemented helper-to-host transport. It is separate from the renderer stream contracts in `crates/notch-protocol`.

## Transport

- 4-byte big-endian length prefix followed by UTF-8 JSON
- maximum body size: 64 KiB
- authenticated OS-local socket/named pipe
- a new 256-bit token for each app start
- token stored only in a user-local runtime descriptor; never argv, environment variables, logs, or SQLite
- Unix verifies the peer's effective UID in addition to token authentication
- bounded client count, queue, field sizes, timeouts, and rate limits

The first connection frame must be:

```json
{
  "type": "auth",
  "v": 1,
  "requestId": "auth",
  "tokenB64": "<descriptor token>"
}
```

The host acknowledges successful authentication, then accepts `ingest` frames. An ingest ACK is emitted only after `HostState` and `AppCore` accept and persist the normalized event; queueing alone is not success.

```json
{
  "type": "ingest",
  "v": 1,
  "requestId": "request-1",
  "payload": {
    "source": "cursor",
    "event": "sessionStart",
    "externalSessionId": "cursor-session-42",
    "label": "cursor session",
    "workspaceLabel": "llm_notch",
    "status": "running",
    "pid": 4242,
    "processStartedAtMs": 1699999999000,
    "occurredAtMs": 1700000000000
  }
}
```

The descriptor is discovered with `notch_ipc::default_runtime_dir`; hooks must not hard-code its path.

## Bounded payload

`IngestPayload` accepts only:

- `source`: `cursor`, `claudeCode`, `codex`, or `generic`
- `event`
- optional `sessionId` / `externalSessionId`
- optional display-only `label` / `workspaceLabel`
- optional `status` / `attention`
- optional redacted `summary` / `toolName`
- optional `pid` / `occurredAtMs`

Unknown fields are rejected on the normalized wire payload. Raw vendor hook JSON is handled only by `llm-notch-hook hook`, which selects a small allowlist and discards prompt text, command bodies, tool input/output, transcript paths, and assistant messages.

## Implemented event mapping

| Payload `event` | Core behavior |
|---|---|
| `sessionStart`, `start`, `sessionUpsert` | Create or update a session |
| `update`, `statusChange` | Update label/workspace/status |
| `sessionEnd`, `end`, `complete`, `fail` | End a session |
| `tool` | Append a redacted tool event |
| `attention` | Set observed local attention and append an event |
| `lifecycle`, `event`, `sessionEvent` | Append a lifecycle event |
| `remove`, `sessionRemove` | Remove the local session |

Process attribution requires both `pid` and `processStartedAtMs`. The host compares that pair with the live process table before registration. Shipped vendor templates provide neither and remain `unknown`; a generic session becomes `exact` only while its explicit pair remains valid.

## Helper modes

Vendor wrappers use fail-open hook mode:

```bash
llm-notch-hook hook \
  --source cursor \
  --vendor-event sessionStart \
  --hook-mode
```

Raw vendor JSON is read from stdin. Hook-mode parse, discovery, or delivery failure exits successfully so monitoring cannot block an agent workflow. The wrapper prints neutral `{}` output where the vendor expects JSON.

Manual normalized emit mode is strict:

```bash
llm-notch-hook emit \
  --source generic \
  --event sessionStart \
  --external-session-id generic-1 \
  --label "Generic agent" \
  --status running \
  --pid 4242 \
  --process-started-at-ms 1700000000000
```

Additional flags are listed by `llm-notch-hook --help`. Emit mode returns failure for invalid input or delivery errors. If the host is unavailable, bounded spool delivery is used.

## Capability and privacy boundaries

- Attention is observation/local acknowledgement only.
- No vendor approve, deny, or answer response is implemented.
- No arbitrary command, path, file body, URL, or network destination crosses this protocol.
- No prompt, command body, tool output, stdout, stderr, or transcript is retained.
- The helper and host perform no cloud/network egress.

Field bounds are defined in `crates/notch-protocol/src/constants.rs` and `crates/notch-ipc/src/limits.rs`.
