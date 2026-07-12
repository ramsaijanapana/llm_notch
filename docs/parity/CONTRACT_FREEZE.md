# Contract freeze v2

**Branch:** `feat/contract-freeze-v2`  
**Owner lane:** Composer (shared contracts)  
**Status:** FROZEN — subsequent lanes must extend, not redefine.

This document lists frozen wire contracts and binding reconciliations from the GPT-5.6 Sol ⇄ Claude Fable planning gate. Implementation of connectors, UI, broker, and detection is **out of scope** for this lane.

## Frozen surfaces

| Domain | Rust module | TS mirror |
|--------|-------------|-----------|
| Session/metrics stream (v1) | `notch-protocol::types` | `src/native/contracts.ts` |
| Connector plan/preview/apply/error/journal | `notch-protocol::connector` | `src/native/contracts.ts` |
| Health probes + user status | `notch-protocol::health` | `src/native/contracts.ts` |
| Decision request/response/delivery | `notch-protocol::decision` | `src/native/contracts.ts` |
| Context open tier + adapter matrix | `notch-protocol::types` | `src/native/contracts.ts` |
| Backup journal entry | `notch-protocol::connector` | `src/native/contracts.ts` |
| Purge scope (`includeBackups` opt-in) | `notch-protocol::purge` | `src/native/contracts.ts` |
| Migration registry numbering | `notch-protocol::migration` | `src/native/contracts.ts` |
| Fail-open hook constants | `notch-protocol::decision` | `src/native/contracts.ts` |

Generated ts-rs bindings live under `crates/notch-protocol/bindings/` for key enums/structs.

## Binding reconciliations (orchestrator)

1. **Health:** Orthogonal probe axes (`installation`, `trust`, `traffic`, `helper`) plus user-facing status `notFound | notInstalled | actionNeeded | waitingFirstEvent | connected | driftDetected | error`. UI summary = first failing probe; diagnostics = full probe vector.
2. **Attribution wire:** `AttributionQuality = exact|shared|heuristic|unknown`. Display maps `unknown` → **"Not attributed"**. Do **not** rename wire to `unavailable`. `MetricAvailability` stays `available|warmingUp|unavailable`.
3. **Detection:** Consent-lite — fixed documented paths only; scan after user clicks Get started. No recursive disk scan.
4. **Decisions UX:** Overlay = entry point; Allow/Deny/answer on focused surface/dashboard. No free-text on non-activating overlay. Broker: no ephemeral payload → no controls. Delivery states: `pending|delivered|effectObserved|expired|failed` with honest microcopy.
5. **Scope:** User-scope default; project opt-in. One confirmation for selected vendors/files with per-file visibility. Codex `/hooks` = guided external `actionNeeded` step only.
6. **Rollback:** Exact restore only when `currentHash == appliedHash`; else additive recovery via normal diff review — never three-way merge editor, never auto-overwrite later edits.
7. **Purge:** History purge keeps backups by default; `includeBackups` explicit opt-in. Delete-all must handle active connectors first.
8. **Parity language:** "Core workflow parity for supported agents" — no unqualified 25-agent or feature-parity claims. Usage/quota omitted.
9. **Plan bodies:** Deferred from launch (preview metadata + display diffs only at first ship).
10. **Multi-file apply:** Per-file atomicity + journal + honest partial success; never claim cross-file atomicity.
11. **Renderer security:** Apply accepts only `planId`; paths in UI are display-only redactions; backend keeps canonical identities.

## Apply / Remove

`apply_connector_change` and `remove_connector` remain **`NotAvailable`**. Shapes are frozen; behavior is not enabled.

## Lane ownership — may touch

| Lane | May modify |
|------|------------|
| Connectors | Implement planner/apply using frozen `ConnectorPlanPreview`, `ConnectorApplyResult`, journal types |
| Detection | Fixed-path scan; populate `HealthProbeResult` |
| Decision broker | `DecisionRequest`, `DecisionResponseRecord`, fail-open constants |
| UI (overlay/dashboard) | Map `ConnectorUserStatus` to presentation; decision surfaces |
| Platform | `MigrationRegistry` records for lane-local schema |

## Lane ownership — must NOT touch without coordination

- Enum variant renames or semantic changes on frozen types
- `PROTOCOL_VERSION` bump without cross-lane agreement
- Renaming `AttributionQuality.unknown` or adding `unavailable` to attribution wire
- Sending canonical file paths or file bodies from renderer to apply commands
- Claiming cross-file atomic apply in wire types or copy

## Verification

```bash
cargo check --workspace
cargo test -p notch-protocol
npm run typecheck
npm run test:run -- src/native/contracts.test.ts src/native/FakeNativeClient.test.ts
```
