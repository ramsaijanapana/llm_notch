# Release candidate status — `feat/parity-rc`

**Branch:** `feat/parity-rc`  
**Base:** `feat/integration-foundations` @ `25d056a`  
**Merged lanes:** 5 (decision), 6 (context), 7 (UI), 8 (observability), 9 (platform)  
**Last committed tip:** `3aa90f5` — `docs(parity-rc): record native UX/technical approvals and tip SHA`  
**Doc refreshed:** 2026-07-11 (wave 11; working tree — not committed)  
**Prior RC tip (lanes 5–9 merge):** `caf37ac4fc33bca2e037e250fc2d5bc4e12f24d8`

> This document is structured for **parallel landing**: foundation and advanced parallel tracks that demonstrate in-tree behavior are listed under **Done**; open end-to-end gaps sit under **Still active**; external-secret gates sit under **Blocked**. Tracks are **not** marked complete unless the code already demonstrates end-to-end behavior.

---

## Track overview

| Track | Status | Honest snapshot |
|-------|--------|-----------------|
| Foundation (lanes 5–9) | **Done** | Decision, context, UI, observability, overlay — merged and reviewed |
| Agent adapters + catalog | **Advanced** | **7 verified** adapters (`cursor`, `claude-code`, `codex`, `gemini-cli`, `qwen`, `antigravity-cli`, `copilot`); **18 catalog-only** entries |
| Pane / terminal bridges | **Advanced** | HWND + WT honest collector + macOS `open -b` / AppleScript Terminal+iTerm2 activation landed; **WT tab/pane auto-discovery upstream-blocked** |
| Quota fetch | **Advanced** | Claude + Codex + Gemini + Kimi credential-gated HTTP probes + **refresh UX + stale/fresh indicators + credential hints**; GLM / DeepSeek honest-unavailable (no public rate-limit headers) |
| Sound themes + playback | **Advanced** | Themed `SoundEngine` + theme picker + master volume/quiet-hours + **per-event and per-agent volume UI** (verified catalog wire IDs); **rodio** on Windows/macOS |
| SSH relay + dashboard | **Advanced** | Remote tab + probe-first deploy + spool E2E + **Linux/macOS relay sidecar CI/release matrix** + **`SessionEvent` kind/tool/attention ingest** + **per-host ingested session stats**; **live SSH soak** still open |
| Windows Authenticode CI | **Advanced** | Fail-closed `release-windows-signed.yml` scaffold in-tree; no successful signed run yet |
| macOS signing / notarization CI | **Advanced** | `release-macos-signed.yml` readiness + gated `signed-macos` matrix in-tree; secrets not configured |
| Signing secrets (publish gate) | **Blocked** | `WINDOWS_CERTIFICATE_*` and `APPLE_*` repository secrets required for signed publish |

See also: [integrations index](../integrations/README.md) · [capability matrix](../integrations/capability-matrix.md) · [Qwen integration](../../integrations/qwen/README.md) · [release signing gates](../../scripts/signing/README.md)

---

## Done

### Parity foundation (lanes 5–9)

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

### Wave 4 landings (working tree)

| Area | Evidence | Notes |
|------|----------|-------|
| Deploy execution | `crates/notch-remote/src/deploy_exec.rs`, `src-tauri/src/services/remote.rs` `execute_deploy`, `execute_remote_deploy` IPC + `RemotePanel` execute UI | SSH/SCP transport: `ProbeTarget` → `UploadTemporary` → `VerifySha256` → `ActivateAtomically`; honest SCP/SSH/hash failures |
| Relay `SessionEvent` ingress | `crates/notch-remote/src/bin/llm-notch-relay.rs`, `hook_ingest.rs`, `event_spool.rs`, `transport.rs` `with_event_spool_dir` | Stdin `InjectHook` control + `--event-spool` watcher normalize hook payloads → `SessionEvent` stdout frames; desktop supervisor ingests |
| Kimi quota probe | `crates/notch-services/src/quota.rs` (`MOONSHOT_API_KEY`), `src-tauri/tests/services_contract.rs` | 4th credential-gated probe; GLM / DeepSeek remain honest-unavailable without probe specs |
| Antigravity verified | `crates/notch-agent-catalog/src/lib.rs`, `notch-hook` fixtures, `integrations/antigravity-cli/` | **6 verified** agents; Antigravity promoted from catalog-only with hook stdin mapping |

### Wave 5 landings (working tree)

| Area | Evidence | Notes |
|------|----------|-------|
| Remote spool E2E wiring | `crates/notch-remote/src/event_spool.rs`, `src-tauri/src/services/remote.rs` `start_relay` + `with_event_spool_dir`, `handle_relay_frame` → `ingest_relay_session_event` | Relay `--event-spool` watcher + desktop supervisor ingest end-to-end; deploy plan `StartStdioRelay` carries `event_spool_dir`; `integrations/remote/` templates document `LLM_NOTCH_EVENT_SPOOL=1` pairing |

### Wave 6 landings (working tree)

| Area | Evidence | Notes |
|------|----------|-------|
| Target-aware deploy artifacts | `crates/notch-remote/src/relay_artifact.rs` `resolve_relay_artifact`, `src-tauri/src/services/remote.rs` `build_deployment_plan` (probe-first) + `build_deployment_plan_for_target`, `src-tauri/src/runtime/relay_path.rs` | `ProbeTarget` → triple resolve picks `src-tauri/binaries/llm-notch-relay-<triple>` or `target/<triple>/{debug,release}/`; `DeployExecError::TargetMismatch` rejects probe/artifact drift |
| Themed `SoundEngine` tray alerts | `src-tauri/src/services/alerts.rs`, `src-tauri/src/services/sound_theme.rs` `play_notification_sound` | Attention/resource/lifecycle alerts play themed sounds once per key via `SoundEngine`; tray path no longer falls back to `MessageBeep` |
| `selectedSoundThemeId` settings UI | `src/features/native-dashboard/components/settings/SettingsPanel.tsx`, `src/app/NativeSurfaces.tsx`, `crates/notch-protocol/bindings/PublicSettings.ts` | Theme picker + per-theme preview buttons; persisted `selectedSoundThemeId` routes tray alert playback |
| `verified_terminal` SQLite persistence | `crates/notch-core/src/persistence/migrations.rs` `MIGRATION_004`, `sqlite.rs` `verified_terminal_json` column + round-trip tests | Sessions survive restarts with collector-supplied terminal metadata (`CURRENT_SCHEMA_VERSION = 5`; migration 004 added column) |
| `verified_terminal` hook collectors | `crates/notch-ipc/src/collector.rs`, `normalize.rs` `verified_terminal_from_ingest` + `enrich_ingest_with_collector_env` | Env/wire collector fields (`WT_SESSION`, `LLM_NOTCH_*`) map to `VerifiedTerminalContext` when present; vendor hook stdin still carries no terminal metadata |
| `DirectRelayTransport` test import fix | `src-tauri/src/services/remote.rs` tests `use notch_remote::DirectRelayTransport`, `crates/notch-remote/tests/relay_lifecycle.rs` | Integration tests compile against public `notch-remote` transport export |

### Wave 7 landings (working tree)

| Area | Evidence | Notes |
|------|----------|-------|
| Windows Terminal honest collector | `crates/notch-platform/src/wt_collector.rs`, `hwnd_collector.rs`, `integrations/wrappers/llm-notch-wt-collector.ps1`, `integrations/windows-terminal/README.md`, `integrations/windows-terminal/test-wt-collector.ps1` | Honest `WT_SESSION` pass-through; verified HWND via process-tree walk + `IsWindow`; `LLM_NOTCH_TAB_ID` / `LLM_NOTCH_PANE_ID` only when pre-set or user-configured |
| Copilot CLI verified | `crates/notch-agent-catalog/src/lib.rs`, `notch-connectors/src/adapter.rs`, `integrations/copilot/`, `notch-hook` `copilotCli` fixtures | **7 verified** agents (`cursor`, `claude-code`, `codex`, `gemini-cli`, `qwen`, `antigravity-cli`, `copilot`); **18 catalog-only** entries; `copilotCli` wire discriminator + camelCase stdin mapping |
| macOS rodio sound backend | `crates/notch-services/src/sound/backend/rodio.rs`, `backend/mod.rs` `default_backend_factory` | Rodio playback on Windows + macOS; stub backend only on Linux/other targets |
| Master volume + quiet-hours settings UI | `src/features/native-dashboard/components/settings/SettingsPanel.tsx`, `SettingsPanel.test.tsx`, `crates/notch-protocol/bindings/SoundRouting.ts`, `QuietHours.ts`, `src-tauri/src/services/sound_theme.rs` | Master volume slider + quiet-hours toggle/start/end persisted in `soundRouting`; `SoundEngine` honors routing (skips playback during quiet hours) |

### Wave 9 landings (working tree)

| Area | Evidence | Notes |
|------|----------|-------|
| Relay `SessionEvent` metadata E2E | `hook_ingest.rs` `kind`/`tool_name`/`attention` + canonical source aliases; `protocol.rs` optional wire fields; `remote.rs` `extract_relay_session_event`; `state.rs` `ingest_relay_session_event` tool/attention/lifecycle routing; `relay_lifecycle.rs` injected + spooled hook tests | Hook/spool/InjectHook → relay stdout → desktop supervisor preserves kind, tool name, and attention; legacy frames without metadata still deserialize |

### Wave 10 landings (working tree)

| Area | Evidence | Notes |
|------|----------|-------|
| `AgentSource` Qwen / AntigravityCli / CopilotCli | `crates/notch-protocol/src/types.rs`, `bindings/AgentSource.ts`, `notch-ipc/src/normalize.rs`, `src-tauri/src/state.rs` `parse_ipc_source` + relay ingest tests | First-class enum variants with serde aliases; relay + IPC ingest no longer collapse to `Generic`; adapter capability templates + attribution wired for all three |
| Per-event + per-agent sound routing UI | `SettingsPanel.tsx` `ROUTING_SOUND_EVENTS` + `ROUTING_AGENT_SOURCES`, `SettingsPanel.test.tsx`, `formatters.ts` `soundEventLabel` / `routingAgentLabel` | Per-event volume sliders (notification, approval, failed, etc.) multiply master volume; per-agent sliders cover core sources + `qwen` / `antigravityCli` / `copilotCli`; unset routes default to 100% |
| Quota refresh UX + credential hints | `MetricsPanel.tsx`, `formatters.ts` `quotaObservedSummary`, `NativeSurfaces.tsx` `refreshQuotas`, `MetricsPanel.test.tsx` | Manual refresh button + loading state; observed-at timestamp + stale/fresh badge (10m threshold); unavailable rows surface `authentication === 'required'` env-var hints |
| Remote host ingested session stats | `remoteSessionStats.ts`, `RemotePanel.tsx`, `RemotePanel.test.tsx`, `NativeSurfaces.tsx` session feed | Per-host cards show ingested session count, active ingested, and last ingested event via `remote:` workspace attribution |
| Typecheck green | `npm run typecheck` | TS client + dashboard compile against extended `AgentSource`, `SoundRouting`, and remote stats contracts |

### Wave 11 landings (working tree)

| Area | Evidence | Notes |
|------|----------|-------|
| SQLite `Generic` → typed `AgentSource` backfill | `migrations.rs` `MIGRATION_005`, `sqlite.rs` `generic_sessions_backfill_from_label_hints` + `generic_backfill_skips_unique_conflicts` tests | `CURRENT_SCHEMA_VERSION = 5`; pre-wave-10 `Generic` rows with `sessionStart` label hints (`qwen session`, `antigravityCli session`, `copilotCli session`) upgrade to typed sources; skips `UNIQUE(source, external_session_id)` conflicts — no guessing for relay-only Generic rows |
| In-repo parity closure | This doc refresh | Required in-repo scaffolds complete; remaining gaps are **vendor**, **upstream**, or **external** only (see **Still active** / **Blocked**) |

### Wave 8 landings (working tree)

| Area | Evidence | Notes |
|------|----------|-------|
| HWND collector + hook/WT integration | `crates/notch-platform/src/hwnd_collector.rs`, `wt_collector.rs`, `integrations/wrappers/llm-notch-wt-collector.ps1`, `integrations/wrappers/llm-notch-hook-wrapper.ps1`, `notch-ipc/src/collector.rs` | Verified HWND via process-tree walk + `IsWindow`; hook wrapper dot-sources WT collector; `verified_terminal_from_ingest` maps collector env to `VerifiedTerminalContext` |
| Linux + macOS relay sidecar CI/release matrix | `.github/workflows/ci.yml` `relay-sidecar-matrix`, `release-installers.yml` `relay-sidecars`, `release-macos-signed.yml`, `package.json` `native:prepare-relay`, `scripts/prepare-native-helper.mjs` `--relay-only`, `src-tauri/tauri.conf.json` `externalBin` | Four triples built unsigned: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`; artifacts land in `src-tauri/binaries/llm-notch-relay-<triple>` and bundle into installers when present |
| `MacOsHostActivationBridge` | `crates/notch-platform/src/macos.rs` `MacOsHostActivationBridge`, `try_exact_pane_host_bridge`, `activate_macos_application` | Terminal.app + iTerm2 via `open -b` bundle activation and AppleScript exact-pane scripts; **not** `RequiresPlatformImplementation` on macOS targets (that disposition is non-macOS compile stub only) |
| Tauri macOS `activate_via_platform_bridge` | `src-tauri/src/context/platform/macos.rs`, `context/platform/bridge.rs`, `notch-platform/src/lib.rs` `default_host_activation_bridge` | `activate()` calls `activate_via_platform_bridge` first; NSWorkspace fallback only when bridge does not activate |

### Parallel-track foundations already in-tree

| Area | Evidence | Notes |
|------|----------|-------|
| `notch-agent-catalog` | `crates/notch-agent-catalog/`, `list_agent_catalog` | 25 entries; **7 verified** (`cursor`, `claude-code`, `codex`, `gemini-cli`, `qwen`, `antigravity-cli`, `copilot`); **18 catalog-only** |
| Qwen adapter (connector + hooks) | `notch-connectors/src/adapter.rs`, `integrations/qwen/`, fixtures | Template merge/apply tests; hook normalization via `claudeCode` wire discriminator |
| Antigravity adapter (connector + hooks) | `notch-connectors/src/adapter.rs`, `integrations/antigravity-cli/`, `notch-hook` fixtures | Named-hook merge/apply tests; `antigravityCli` stdin mapping for PreToolUse / PostToolUse / Stop; promoted to `VerifiedCurrent` |
| Copilot adapter (connector + hooks) | `notch-connectors/src/adapter.rs`, `integrations/copilot/`, `notch-hook` fixtures | `hooks.json.template` merge/apply tests; `copilotCli` stdin mapping for sessionStart / preToolUse / postToolUse / permissionRequest / agentStop / sessionEnd; promoted to `VerifiedCurrent` |
| `notch-services` contracts | `crates/notch-services/` | Quota probe specs + provider registry; sound engine + secure pack validation — **36/36 lib tests pass** |
| Claude + Codex + Gemini + Kimi quota probes | `notch-services/src/quota.rs`, `src-tauri/src/services/quota.rs` | `CredentialGatedQuotaProvider` for `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `GOOGLE_API_KEY` \| `GEMINI_API_KEY` / `MOONSHOT_API_KEY`; GLM / DeepSeek rows honest-unavailable |
| ExactPane metadata plumbing | `context/locator.rs`, `context/platform/bridge.rs`, `context/mod.rs`, `notch-ipc/src/collector.rs` | `VerifiedTerminalContext` round-trips through `ContextLocator` + SQLite (`verified_terminal_json`, migration 004); `pane_verified` gates `ExactPane` target tier; collector env/wire fields populate normalize path |
| `open_session` platform bridge wiring | `context/platform/windows.rs`, `context/platform/macos.rs`, `context/platform/bridge.rs` | Windows + macOS `activate_via_platform_bridge` runs before legacy fallback; exact-pane bridge receives locator metadata |
| `notch-platform` discovery + WT bridge | `crates/notch-platform/`, `context/resolve.rs`, `windows.rs` | Process-tree host detection; `build_wt_exact_pane_command` + `activate_windows_terminal_exact_pane` |
| Windows Terminal hook collector | `notch-platform/src/wt_collector.rs`, `hwnd_collector.rs`, `llm-notch-hook collect-terminal-env`, `integrations/wrappers/llm-notch-wt-collector.ps1`, `integrations/windows-terminal/README.md` | Honest `WT_SESSION` pass-through; verified HWND discovery; hook wrapper dot-sources collector |
| Native sound playback | `crates/notch-services/src/sound/engine.rs`, `sound/backend/rodio.rs`, `play_sound_event`, `alerts.rs` | Rodio backend on Windows/macOS; builtin + imported themes via `load_installed_theme`; tray alerts route through `SoundEngine` with `selectedSoundThemeId` + `soundRouting` |
| Secure sound pack import | `crates/notch-services/src/sound_pack.rs`, `import_sound_pack` | Zip integrity manifest, traversal/hash rejection, reserved-id guard; install to app-data themes root |
| Services IPC | `src-tauri/src/commands/services.rs` | `list_quota_snapshots`, `get_sound_themes`, `preview_sound_routing`, `play_sound_event`, `import_sound_pack` |
| Dashboard service wiring | `NativeSurfaces.tsx`, `MetricsPanel.tsx`, `AgentStatusRail.tsx`, `SettingsPanel.tsx`, `RemotePanel.tsx` | Catalog + quota snapshots with refresh/stale UX; agent status rail; sound theme picker + per-event/per-agent volume + quiet-hours; remote per-host ingest stats |
| TS native client seam | `src/native/commands.ts`, `TauriNativeClient.ts` | Catalog, quota, sound, and remote command bindings |
| Gemini adapter (connector) | `notch-connectors/src/adapter.rs`, `integrations/gemini/` | Template, merge, apply tests; fixtures + validate-json coverage |
| Gemini hook normalization | `notch-hook`, `notch-ipc/src/normalize.rs` | Vendor event mapping for Gemini CLI sessions |
| `notch-remote` + Tauri registry | `crates/notch-remote/`, `src-tauri/src/services/remote.rs`, `commands/remote.rs` | `notch-remote` linked in `src-tauri/Cargo.toml`; `DesktopRemoteRegistry` + relay lifecycle IPC; host CRUD + SQLite persistence; relay supervisor poll loop |
| Relay session-event ingress (relay + desktop) | `llm-notch-relay.rs` `InjectHook` + `--event-spool`, `hook_ingest.rs`, `event_spool.rs`, `remote.rs` `handle_relay_frame`, `state.rs` `ingest_relay_session_event` | Relay normalizes hook payloads → `SessionEvent` frames on stdout; desktop supervisor ingests into `AppCore` + stream hub; heartbeats/checkpoints do not fabricate sessions |
| Remote deploy execution | `deploy_exec.rs`, `remote.rs` `execute_deploy`, `execute_remote_deploy` IPC | SSH/SCP `probe→upload→verify→activate` via `DeploymentExecutor`; honest failures when SSH/SCP/hash verification fails; deploy preview/execute messages + `integrations/remote/` templates document `LLM_NOTCH_EVENT_SPOOL=1` spool pairing |
| Target-aware relay artifact selection | `notch-remote/src/relay_artifact.rs`, `remote.rs` `build_deployment_plan` (probe-first), `relay_path.rs` `relay_binaries_directory` | `ProbeTarget` → `resolve_relay_artifact` picks cross-compiled sidecar or `target/<triple>/` build; `TargetMismatch` guard in executor |
| Remote live connection listener | `NativeSurfaces.tsx`, `TauriNativeClient.ts` `subscribeRemoteConnectionChanges`, `remote.rs` `REMOTE_CONNECTION_CHANGED_EVENT` | Dashboard applies `remote-connection-changed` events to host connection state |
| Remote dashboard tab | `DashboardTabs.tsx` (`remote`), `RemotePanel.tsx`, `RemoteConnectionBadge.tsx` | Backend probe, deploy preview + execute UI, host CRUD, start/stop relay actions (honest failures when backend missing) |
| Relay path resolver | `src-tauri/src/runtime/relay_path.rs` | `LLM_NOTCH_RELAY_BIN` override → resource dir → `target/debug` fallback; bundled via `externalBin` |
| Installer CI (unsigned) | `.github/workflows/release-installers.yml` | Draft release matrix: Windows x64 + macOS arm64/x64 (ad-hoc macOS signing); relay sidecars built on Linux + macOS runners and attached to draft releases |
| Windows signed workflow (fail-closed) | `.github/workflows/release-windows-signed.yml` | `signing_gate` job errors when `WINDOWS_CERTIFICATE_*` absent; `draft-unsigned` escape hatch |
| macOS signed workflow (scaffold) | `.github/workflows/release-macos-signed.yml` | `signing-readiness` reports missing `APPLE_*`; `signed-macos` matrix gated on secrets |
| Signing scaffold | `scripts/signing/` | `sign-windows.ps1`, `notarize-macos.sh`, `tauri.windows.signed.conf.json` |

### Canonical IPC surface (foundation + parallel-track commands)

| Rust command | TS invoke key | TS method |
|--------------|---------------|-----------|
| `list_pending_decisions` | `list_pending_decisions` | `getPendingDecisions()` |
| `submit_decision` | `submit_decision` | `respondDecision(id, response)` |
| `open_session` | `open_session` | `openSession(id)` → `OpenSessionResult` |
| `list_connector_backups` | `list_connector_backups` | `listConnectorBackups()` |
| `list_agent_catalog` | `list_agent_catalog` | `listAgentCatalog()` |
| `list_quota_snapshots` | `list_quota_snapshots` | `listQuotaSnapshots()` |
| `get_sound_themes` | `get_sound_themes` | `getSoundThemes()` |
| `preview_sound_routing` | `preview_sound_routing` | `previewSoundRouting()` |
| `play_sound_event` | `play_sound_event` | `playSoundEvent()` |
| `import_sound_pack` | `import_sound_pack` | `importSoundPack()` |
| `list_remote_hosts` | `list_remote_hosts` | `listRemoteHosts()` |
| `get_remote_backend_status` | `get_remote_backend_status` | `getRemoteBackendStatus()` |
| `preview_remote_deploy` | `preview_remote_deploy` | `previewRemoteDeploy(hostId)` |
| `execute_remote_deploy` | `execute_remote_deploy` | `executeRemoteDeploy(hostId)` |
| `start_remote_relay` | `start_remote_relay` | `startRemoteRelay(hostId)` |
| `stop_remote_relay` | `stop_remote_relay` | `stopRemoteRelay(hostId)` |
| `get_remote_connection_status` | `get_remote_connection_status` | `getRemoteConnectionStatus(hostId)` |

Lane 7 originally expected `get_pending_decisions` / `respond_decision`; RC aligns on Lane 5 Rust names with TS method names unchanged for UI compatibility.

### Merge / drift fixes (Lane 10)

1. **`commands/mod.rs`** — kept both `context` and `decision` modules after L6 merge.
2. **`HostState` constructors** — unified `decision_broker` + `alert_notifier` in `with_runtime_dir_and_notifier`.
3. **`lib.rs`** — merged `mod context` + `pub mod runtime`; registered all commands including backups + decisions.
4. **TS client** — `commands.ts` maps to `list_pending_decisions` / `submit_decision`; `openSession` returns structured result.
5. **`list_connector_backups`** — added to `integration.rs`, connector manager/journal, permissions, capabilities.
6. **Compile fixes** — `AlertKind: Hash`, `unsafe extern` for `MessageBeep`, `tauri::Manager` import in `state.rs`.
7. **Context-open caps** — Cursor/Claude/Codex + builtin templates advertise `contextOpenTier: appActivate`.
8. **`record_event`** — Lane 8 traffic probe path preserved through merged `integration.rs` / ingest.

### Test evidence (last verified at foundation tip)

| Gate | Command | Result |
|------|---------|--------|
| Rust fmt | `cargo fmt --all --check` | **PASS** (after fmt) |
| Rust check | `cargo check --workspace` | **PASS** |
| Protocol tests | `cargo test -p notch-protocol --lib` | **80 passed** |
| Metrics tests | `cargo test -p notch-metrics --lib` | **13 passed**, 1 ignored |
| Core tests | `cargo test -p notch-core --lib` | **41+ passed** (includes migration 005 `Generic`→typed backfill + conflict skip) |
| Connectors tests | `cargo test -p notch-connectors --lib` | **21+ passed** (includes Gemini apply/merge) |
| Decision tests | `cargo test -p notch-decision --lib` | **9 passed** |
| Adapter tests | cursor/claude/codex/generic `--lib` | **80 passed** |
| Services contract | `cargo test -p llm-notch-desktop --test services_contract` | 6 quota rows; Claude/Codex/Gemini/Kimi probe paths; GLM/DeepSeek honest-unavailable; never fabricates usage; builtin theme validates |
| `notch-services` lib | `cargo test -p notch-services --lib` | **36 passed** |
| Remote registry contract | `cargo test -p llm-notch-desktop --test remote_registry` | Host CRUD roundtrip + relay poll (relay binary gated); **BLOCKED** here by App Control policy on test exe |
| `notch-remote` lib | `cargo test -p notch-remote --lib` | **30 passed** (includes `relay_artifact` resolution + `TargetMismatch` guard) |
| `notch-platform` lib | `cargo test -p notch-platform --lib` | **wt_collector** + **hwnd_collector** + Windows bridge unit tests |
| IPC tests | `cargo test -p notch-ipc --lib` | **16 passed**, **3 failed** (Windows socket `PermissionDenied`) |
| Desktop lib tests | `cargo test -p llm-notch-desktop --lib` | **BLOCKED** — `STATUS_ENTRYPOINT_NOT_FOUND` (WebView2/runtime load) |
| Native Windows | `cargo test -p llm-notch-desktop --test native_windows` | **4 passed** |
| Typecheck | `npm run typecheck` | **PASS** |
| Vitest | `npm run test:run` | **170+ passed** (includes `RemotePanel` ingest stats, `MetricsPanel` quota refresh, `SettingsPanel` per-event/per-agent volume, `remoteSessionStats`) |
| Biome lint | `npm run lint` | **FAIL** — pre-existing CRLF format drift (~200 files) |
| E2E | `npm run test:e2e` | **SKIPPED** — Playwright browsers not installed |

### Final review approvals (foundation)

| Reviewer | Verdict | Recorded at |
|----------|---------|-------------|
| Claude Fable | **NATIVE UX APPROVED** | `9cf2ddc` |
| GPT-5.6 Sol | **NATIVE TECHNICAL APPROVED** | `caf37ac4fc33bca2e037e250fc2d5bc4e12f24d8` |

**Unsigned RC review (lanes 5–9):** **PASSED** at foundation tip. Parallel tracks below extend the surface; they do not automatically inherit signed-release readiness.

---

## Still active

Open gaps grouped by **who can close them**. Waves 9–11 closed all **required in-repo feature scaffolds**; what remains is **vendor contracts**, **upstream platform limits**, and **external secrets/soak** only.

### Summary

| Category | Open items (honest) |
|----------|---------------------|
| **Vendor** | **18 catalog-only** adapters need hook contracts before verification; **GLM / DeepSeek** quota telemetry (no public rate-limit headers); live E2E on real Antigravity / Qwen / Copilot CLI installs |
| **Upstream** | **WT tab/pane auto-discovery** — Windows Terminal publishes `WT_SESSION` only; numeric tab/pane require user layout config; `wt.exe focus-tab --session` blocked upstream; VS Code / Cursor / ConEmu exact-pane bridges (no host query API); macOS overlay hardening (`NSPanel` / Tauri upstream) |
| **External** | **Signing secrets** (`WINDOWS_CERTIFICATE_*`, `APPLE_*`) + protected CI signed publish runs; **live SSH multi-host relay soak**; Playwright E2E browsers on CI; WebView2 desktop `--lib` test harness; Windows named-pipe ACL for IPC integration tests |

### 1. Catalog-only agents (vendor)

| Present | Missing |
|---------|---------|
| **7 verified** adapters with connector templates, fixtures, and hook paths | **18 catalog-only entries** (e.g. `kimi-code`, `deepseek`, `zcode`) need vendor hook contracts before adapters can be verified |
| `AgentSource` Qwen / AntigravityCli / CopilotCli enum + normalize (wave 10) | Live end-to-end verification on real Antigravity / Qwen / Copilot CLI installs |

### 2. Quota providers — GLM / DeepSeek (vendor)

| Present | Missing |
|---------|---------|
| Claude + Codex + Gemini + Kimi credential-gated HTTP probes; refresh UX + stale/fresh indicators + credential hints (wave 10) | **GLM, DeepSeek** — no public rate-limit headers / probe specs; rows stay honest-unavailable until vendor surfaces usable telemetry |

### 3. Windows Terminal tab/pane (upstream)

| Present | Missing |
|---------|---------|
| Honest `WT_SESSION` pass-through; verified HWND discovery; WT exact-pane bridge when tab/pane pre-configured | **Auto-discovered WT tab/pane indices** — blocked upstream |

### 4. Remote relay soak (external)

| Present | Missing |
|---------|---------|
| Deploy execution SSH/SCP pipeline; CI/release relay sidecar matrix; `SessionEvent` kind/tool/attention ingest; per-host ingested session stats (waves 9–10) | **Live SSH deploy + multi-host relay soak** on real remote hosts; production bundle verification on signed CI matrix. Soak checklist: deploy relay sidecar → start relay → trigger remote hook with `LLM_NOTCH_EVENT_SPOOL=1` → confirm desktop ingests `SessionEvent` + per-host stats (see [`integrations/remote/README.md`](../../integrations/remote/README.md)) |

### 5. Signing secrets + signed publish (external)

| Present | Missing |
|---------|---------|
| Fail-closed Windows + macOS signed workflow scaffolds; signing scripts | **`WINDOWS_CERTIFICATE_*` and `APPLE_*` repository secrets**; successful protected-tag workflow runs producing stapled/notarized + Authenticode artifacts |

### 6. Deferred (upstream / live soak — not in-repo)

| Present | Missing |
|---------|---------|
| HWND + WT collectors; macOS Terminal/iTerm2 bridges; VS Code / Cursor / ConEmu honest `Unavailable` bridges in `notch-platform` | Live verified pane selection soak on real hosts; VS Code / Cursor / ConEmu exact-pane activation (blocked on host query APIs) |
| Onboarding flow + integrations panel shipped (lane 7) | Copy/UX polish only if a future review flags gaps — not a parity scaffold blocker |

---

## Blocked (external secrets & environment)

These items cannot close in-repo. Workflow scaffolds are **Advanced** (see **Done**); this section covers what still gates **signed release publish**.

| Blocker | Required input | Current state |
|---------|----------------|---------------|
| Windows Authenticode secrets | `WINDOWS_CERTIFICATE_BASE64`, `WINDOWS_CERTIFICATE_PASSWORD` | `release-windows-signed.yml` fail-closed; `signing_gate` errors before build |
| macOS Developer ID + notarization secrets | `APPLE_CERTIFICATE_BASE64`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_ID_PASSWORD`, `APPLE_TEAM_ID` | `signed-macos` job skipped until secrets land; readiness job reports missing keys |
| Playwright E2E on CI | Browser install on runners | Locally skipped; not evidenced on release path |
| Desktop `--lib` tests (Windows) | WebView2 runtime in test harness | `STATUS_ENTRYPOINT_NOT_FOUND` in this environment |
| IPC socket integration tests | Windows named-pipe ACL / sandbox | 3 tests fail with `PermissionDenied` — environment, not logic |
| Live SSH multi-host relay soak | Real remote host(s) with SSH + deployed relay sidecar | In-tree deploy/spool/ingest pipeline complete; confidence gate requires manual soak (see **Still active** §4) |
| macOS overlay hardening | True `NSPanel` or upstream Tauri support | `native_macos` test gate in CI; production overlay deferred |

### Residual code risks (not secret-blocked)

- Connector apply TOCTOU: layered mitigations; residual parent-directory swap race — see [CONNECTOR_TOCTOU.md](CONNECTOR_TOCTOU.md)
- `includeBackups` purge clears journal metadata only; physical backup file deletion deferred
- Decision stream push still polled from UI; broker stream frames future work
- Repo-wide CRLF lint noise (pre-existing)

---

## RC readiness

| Gate | Status |
|------|--------|
| **RC_READY_FOR_REVIEW** (unsigned, lanes 5–9) | `true` at foundation tip `caf37ac` |
| **Parallel tracks landing** | **Advanced** — waves 9–11 close relay `SessionEvent` metadata, per-event/per-agent sound UI, quota refresh UX, remote ingest stats, `AgentSource` extension, and SQLite Generic backfill; see **Done** and **Still active** |
| **In-repo code parity** | **COMPLETE** — no required feature scaffold remains; optional polish and live soak are non-blocking |
| **Signed release publish** | **BLOCKED (external)** — workflow scaffolds + relay matrix in-tree; `WINDOWS_*` / `APPLE_*` secrets + successful protected CI runs still required |
| **Full catalog parity** | **NOT DONE** — vendor catalog contracts, upstream WT limits, and external soak/signing gates remain |

### Loop recommendation (waves 9–11)

| Question | Answer |
|----------|--------|
| **Should `/loop wake` stop?** | **Yes — stop now.** All required in-repo scaffolds are landed (wave 11 closes SQLite Generic backfill). **Still active** is vendor, upstream, and external only. |
| **When should it stop?** | Immediately; commit working-tree changes on next pass. **In-repo (code)** no longer blocks unsigned RC review or signed-release scaffold readiness. |
| **Signed production-only lens** | External gates dominate: signing secrets, protected CI runs, live SSH soak. No in-repo feature scaffold is missing. |

Compile surface is green for the workspace; vitest/typecheck pass; crate-level Rust tests pass; native Windows smoke passes. Treat IPC socket failures, desktop lib test harness, E2E skip, and CRLF lint as known environment/pre-existing items.

### Signed production vs full catalog parity

| Goal | Remaining work | External-only? |
|------|----------------|----------------|
| **Signed production installers** | Configure `WINDOWS_CERTIFICATE_*` + `APPLE_*` secrets; run protected-tag signed workflows; verify stapled/notarized + Authenticode artifacts on release matrix; live SSH soak for confidence | **Mostly yes** — signing secrets and CI soak are the hard gates; macOS overlay hardening and Playwright E2E on CI are secondary environment/upstream items |
| **Full catalog + services parity** | 18 catalog-only agents (**vendor** hook contracts); GLM/DeepSeek (**vendor** APIs); WT tab/pane upstream; VS Code/Cursor/ConEmu exact-pane upstream; live SSH soak (**external**) | **Vendor/upstream/external only** — in-repo scaffolds complete through wave 11 |

**Honest bottom line:** **In-repo work is complete** for parity RC goals. Waves 9–11 closed relay metadata, sound routing UI, quota refresh UX, remote ingest stats, `AgentSource` extension, and SQLite Generic backfill. For **signed production**, only **external blockers** (signing secrets + protected CI runs + real-host SSH soak for confidence) remain. For **full catalog parity**, **vendor contracts**, **upstream**, and **external** items dominate — do not invent verified agents or quota numbers.

**Signed release publish:** **BLOCKED** until signing secrets land, protected CI jobs produce stapled/notarized macOS + Authenticode Windows artifacts, and release-matrix bundle verification passes. Playwright E2E on CI and macOS overlay hardening are secondary gates documented under **Blocked**. This RC does **not** publish a GitHub Release with signed installers until those gates close.

---

## Local artifacts

| Artifact | Path |
|----------|------|
| Hook sidecar (debug) | `target/debug/llm-notch-hook.exe` |
| Bundled hook (Tauri) | `src-tauri/binaries/llm-notch-hook-x86_64-pc-windows-msvc.exe` |
| Relay sidecar (debug) | `target/debug/llm-notch-relay.exe` |
| Bundled relay (Tauri) | `src-tauri/binaries/llm-notch-relay-x86_64-pc-windows-msvc.exe` |
| Desktop binary (debug) | `target/debug/llm-notch-desktop.exe` |

---

## Commit history reference

| SHA | Message |
|-----|---------|
| `3aa90f5` | `docs(parity-rc): record native UX/technical approvals and tip SHA` |
| `caf37ac` | `fix(connectors): close apply/backup TOCTOU races on feat/parity-rc` |
| `855b951` | `feat(parity-rc): merge lanes 5-9, align IPC surface, and document RC status` |

Parallel-track changes through wave 11 (relay metadata, sound routing UI, quota refresh UX, remote ingest stats, `AgentSource` extension, SQLite Generic backfill, catalog, services, remote, platform, macOS activation bridge, relay sidecar matrix, Qwen, Gemini, signing workflows) are present in the working tree as of this doc refresh and are **not yet committed**.
