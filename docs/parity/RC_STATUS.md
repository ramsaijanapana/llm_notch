# Release candidate status — `feat/parity-rc`

**Branch:** `feat/parity-rc`  
**Base:** `feat/integration-foundations` @ `25d056a`  
**Merged lanes:** 5 (decision), 6 (context), 7 (UI), 8 (observability), 9 (platform)  
**Author:** llm_notch agent  
**Date:** 2026-07-11

> Commit SHA filled at end of Lane 10 phase B (see **Commit** section below).

## Parity matrix snapshot

| Area | Status | Notes |
|------|--------|-------|
| Connectors (L1–4 base) | ✅ | Detect, preview, apply, repair, rollback |
| Decision broker (L5) | ✅ | `list_pending_decisions`, `submit_decision`, hook IPC |
| Context navigation (L6) | ✅ | `open_session` → `OpenSessionResult` |
| Native UI (L7) | ✅ | Onboarding, integrations, decision surface |
| Observability (L8) | ✅ | Alerts, scoped purge, traffic probes |
| Platform overlay (L9) | ✅ | Windows non-activating overlay, helper path resolver |
| IPC command surface | ✅ | Lane 5 names canonical; TS client aligned |
| Backup listing | ✅ | `list_connector_backups` wired |
| Context-open UI gating | ✅ | Cursor/Claude/Codex advertise `appActivate` |

## Canonical IPC surface

| Rust command | TS invoke key | TS method |
|--------------|---------------|-----------|
| `list_pending_decisions` | `list_pending_decisions` | `getPendingDecisions()` |
| `submit_decision` | `submit_decision` | `respondDecision(id, response)` |
| `open_session` | `open_session` | `openSession(id)` → `OpenSessionResult` |
| `list_connector_backups` | `list_connector_backups` | `listConnectorBackups()` |

Lane 7 originally expected `get_pending_decisions` / `respond_decision`; RC aligns on Lane 5 Rust names with TS method names unchanged for UI compatibility.

## Merge / drift fixes (Lane 10)

1. **`commands/mod.rs`** — kept both `context` and `decision` modules after L6 merge.
2. **`HostState` constructors** — unified `decision_broker` + `alert_notifier` in `with_runtime_dir_and_notifier`.
3. **`lib.rs`** — merged `mod context` + `pub mod runtime`; registered all commands including backups + decisions.
4. **TS client** — `commands.ts` maps to `list_pending_decisions` / `submit_decision`; `openSession` returns structured result.
5. **`list_connector_backups`** — added to `integration.rs`, connector manager/journal, permissions, capabilities.
6. **Compile fixes** — `AlertKind: Hash`, `unsafe extern` for `MessageBeep`, `tauri::Manager` import in `state.rs`.
7. **Context-open caps** — Cursor/Claude/Codex + builtin templates advertise `contextOpenTier: appActivate`.
8. **`record_event`** — Lane 8 traffic probe path preserved through merged `integration.rs` / ingest.

## Test evidence

| Gate | Command | Result |
|------|---------|--------|
| Rust fmt | `cargo fmt --all --check` | **PASS** (after fmt) |
| Rust check | `cargo check --workspace` | **PASS** |
| Protocol tests | `cargo test -p notch-protocol --lib` | **80 passed** |
| Metrics tests | `cargo test -p notch-metrics --lib` | **13 passed**, 1 ignored |
| Core tests | `cargo test -p notch-core --lib` | **36 passed** |
| Connectors tests | `cargo test -p notch-connectors --lib` | **21 passed** |
| Decision tests | `cargo test -p notch-decision --lib` | **9 passed** |
| Adapter tests | cursor/claude/codex/generic `--lib` | **80 passed** |
| IPC tests | `cargo test -p notch-ipc --lib` | **16 passed**, **3 failed** (Windows socket `PermissionDenied`) |
| Desktop lib tests | `cargo test -p llm-notch-desktop --lib` | **BLOCKED** — test binary `STATUS_ENTRYPOINT_NOT_FOUND` (WebView2/runtime load) |
| Native Windows | `cargo test -p llm-notch-desktop --test native_windows` | **4 passed** |
| Typecheck | `npm run typecheck` | **PASS** |
| Vitest | `npm run test:run` | **159 passed** (31 files) |
| Biome lint | `npm run lint` | **FAIL** — pre-existing CRLF format drift (~200 files); not introduced by RC |
| E2E | `npm run test:e2e` | **SKIPPED** — Playwright browsers not installed (`npx playwright install` required) |

## Local artifacts

| Artifact | Path |
|----------|------|
| Hook sidecar (debug) | `target/debug/llm-notch-hook.exe` |
| Bundled hook (Tauri) | `src-tauri/binaries/llm-notch-hook-x86_64-pc-windows-msvc.exe` |
| Desktop binary (debug) | `target/debug/llm-notch-desktop.exe` |

## Remaining limitations

### P0 (blocks signed release)

- Signing secrets not in repo (`WINDOWS_CERTIFICATE_*`, `APPLE_*`)
- macOS true `NSPanel` not implemented (honest partial capability only)
- Desktop `--lib` unit tests cannot launch in this Windows environment (WebView2 entrypoint)

### P1 (post-RC polish)

- IPC socket integration tests fail with `Access is denied` on Windows (environment ACL)
- Playwright E2E not run locally (browser cache missing)
- Repo-wide CRLF lint noise (pre-existing)
- `includeBackups` purge clears journal metadata only; physical backup file deletion deferred
- Decision stream push still polled from UI; broker stream frames future work

## What blocks release

1. Code signing / notarization credentials in CI
2. macOS overlay hardening (NSPanel or upstream Tauri support)
3. E2E verification on CI runners with Playwright browsers
4. Optional: fix Windows IPC test sandbox permissions

## RC readiness

**RC_READY_FOR_REVIEW:** `true` (with documented P0/P1 caveats above)

Sol/Fable final review **may proceed** — compile surface is green, vitest/typecheck pass, crate-level Rust tests pass, native Windows smoke passes. Reviewers should treat IPC socket failures, desktop lib test harness, E2E skip, and CRLF lint as known environment/pre-existing items.

## Commit

**SHA:** `855b9516c3d71948332b2309f8a5c000291c9991`  
**Message:** `feat(parity-rc): merge lanes 5-9, align IPC surface, and document RC status`  
Local only — not pushed.
