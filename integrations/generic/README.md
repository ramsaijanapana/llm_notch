# Generic agent integration (protocol v1)

**Nothing here is installed automatically.** Third-party agents emit bounded events to the local `llm-notch-hook` helper using explicit CLI calls or their own IPC client that speaks the same ingest wire format documented in [`docs/integrations/protocol.md`](../../docs/integrations/protocol.md).

## Quick start

1. Build or install the signed `llm-notch-hook` helper and start the llm_notch desktop host.
2. Copy patterns from [`emit-examples.sh`](./emit-examples.sh) or [`emit-examples.ps1`](./emit-examples.ps1).
3. Declare optional client capabilities (including ingest ACK support) using `notch-adapters-generic::GenericClientCapabilities`.

## Protocol version

Generic protocol **v1** matches `GENERIC_PROTOCOL_VERSION` in `crates/notch-adapters/generic`. Example fixtures live under [`integrations/fixtures/protocol/`](../fixtures/protocol/).

## Optional capability declaration

Clients may opt into documented ingest ACK semantics without enabling decision response:

```rust
use notch_adapters_generic::{GenericClientCapabilities, capabilities_with_ack};

let caps = capabilities_with_ack(&GenericClientCapabilities {
    supports_response_ack: true,
    emits_attention: true,
    declares_process_root: true,
    ..Default::default()
});
```

Decision response remains `false` in V1 — ACK only confirms host persistence.

## Examples

| Script | Purpose |
|--------|---------|
| `emit-examples.sh` | POSIX shell documentation samples |
| `emit-examples.ps1` | PowerShell documentation samples with optional PID attribution |

Run examples only after placing `llm-notch-hook` on `PATH` or setting `LLM_NOTCH_HOOK_BIN`.

## SDK reference

- Rust crate: `notch-adapters-generic`
- Extended guide: [`docs/integrations/generic-protocol.md`](../../docs/integrations/generic-protocol.md)
