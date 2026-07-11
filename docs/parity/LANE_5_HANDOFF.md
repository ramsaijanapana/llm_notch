# Lane 5 — Decision broker HANDOFF

**Branch:** `feat/lane-5-decision`  
**Worktree:** `llm_notch-wt-decision`  
**Base:** `feat/integration-foundations` @ `25d056a`  
**Status:** `LANE_5_COMPLETE`

## Summary

Shipped interactive decision broker for Claude Code permission/approval hooks: capability-gated `DecisionRequest` flow with nonce idempotency, honest delivery states, fail-open hook timeout, and Tauri commands for the focused decision surface.

## New crate — `notch-decision`

| Module | Purpose |
|--------|---------|
| `DecisionBroker` | Core state machine: `pending → chosen → delivered → effectObserved` (or `expired`/`failed`) |
| `adapter` | Builds verified Claude stdout via `notch-adapters-claude-code::build_decision_response` |
| `store` | SQLite `decision_audit` table + `MigrationLane::Decisions` registry record |
| `types` | Internal wait/reply payloads (ephemeral vendor context stays backend-only) |

### Broker guarantees

- **Never spool** decision waits (IPC `DecisionWait` is not replayed from event spool)
- **Never replay expired** decisions; late submit returns `Expired`
- **Never fallback to Allow**; timeout/failure returns neutral `{}`
- **`hasActionablePayload`**: UI must hide controls when false (unknown Claude version, Cursor/Codex V1)
- **Idempotent by nonce** (`requestId`); connection binding via `connectionId`
- **`effectObserved`**: only via explicit `observe_effect` with correlatable evidence (not auto-claimed)

## IPC extensions — `notch-ipc`

Wire messages (not frozen protocol v1 — ingest wire v1 extension):

- `DecisionWait` — hook → host (interactive, never spooled)
- `DecisionReply` — host → hook (`stdoutJson`, `deliveryState`)

`IngestClient::request_decision` waits `DECISION_FAIL_OPEN_TIMEOUT_MS` (2s) then fail-opens.

`IngestServerConfig.decision_wait_tx` routes waits to broker (host-owned `mpsc` channel).

## Hook changes — `notch-hook`

Vendor hook mode (`hook --source … --vendor-event … --hook-mode`):

1. Claude `PermissionRequest` / `ExitPlanMode` → plan interactive flow via `decision::plan_interactive_decision`
2. Deliver attention ingest + `request_decision` IPC
3. Print broker stdout or `{}` (always exit 0)
4. Non-respondable events → ingest only + `{}`

Dependencies added: `notch-adapters-claude-code`, `chrono`.

## Tauri commands

| Command | Args | Returns |
|---------|------|---------|
| `list_pending_decisions` | — | `DecisionRequest[]` |
| `submit_decision` | `request_id`, `DecisionResponse` | `DecisionResponseRecord` |

Registered in `build.rs`, dashboard capability (`allow-list-pending-decisions`, `allow-submit-decision`).

`DecisionBroker` is `app.manage()`'d and wired in `HostState::run_ipc` decision channel.

## UI lane integration

```typescript
// Dashboard focused decision surface
const pending = await invoke<DecisionRequest[]>('list_pending_decisions')
// Render controls only when request.hasActionablePayload === true
await invoke('submit_decision', {
  requestId: request.id,
  response: { type: 'action', action: 'allow' }, // or deny
})
```

Honest delivery microcopy maps to `DecisionResponseRecord.deliveryState`:

- `pending` — awaiting user or hook wait
- `delivered` — stdout sent to hook (not vendor ACK)
- `effectObserved` — correlatable evidence recorded
- `expired` / `failed` — neutral `{}` was used; say so plainly

Gate approve/deny on `adapter.decisionResponse && request.hasActionablePayload`.

## Cargo workspace

```toml
members = [ ..., "crates/notch-decision" ]
[workspace.dependencies]
notch-decision = { path = "crates/notch-decision" }
```

`src-tauri/Cargo.toml`: `notch-decision.workspace = true`

## Tests

```bash
cargo test -p notch-decision   # 9 passed (ack/timeout/duplicate/fail-open/expired/not-actionable)
cargo test -p notch-hook         # 10 passed (interactive plan + existing ingest tests)
cargo check -p notch-decision -p notch-ipc -p notch-hook -p notch-core
```

`notch-ipc` server integration tests may fail on Windows with socket `PermissionDenied` in some environments (pre-existing platform flake).

## Blockers / follow-ups

| Item | Owner | Notes |
|------|-------|-------|
| `llm-notch-desktop` full build | Platform | Requires `src-tauri/binaries/llm-notch-hook-x86_64-pc-windows-msvc.exe` (Lane 1 handoff) |
| UI decision surface wiring | Lane 7 | Call new Tauri commands; respect `hasActionablePayload` |
| `observe_effect` Tauri command | Lane 5 optional / UI | Broker API exists; wire when vendor effect correlation lands |
| Cursor/Codex decision response | Adapter lanes | Observation-only V1; broker returns `CapabilityDisabled` |
| Stream push for new decisions | UI/Platform | Poll `list_pending_decisions` for now |

No frozen protocol enum changes. `MigrationLane::Decisions` v1 registered on first broker open.
