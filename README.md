# llm_notch

llm_notch is a local-first Tauri 2 desktop monitor for LLM agent sessions. The native app has two windows:

- `overlay`: a transparent, non-activating compact/peek island
- `dashboard`: sessions, local metrics, integration capability status, and settings

The marketing site is rendered only by a normal browser preview. Tauri windows route through `DesktopApp` and never show the marketing page.

## Architecture

- `crates/notch-protocol`: strict protocol-v1 Rust contracts mirrored in `src/native/contracts.ts`
- `crates/notch-core`: session registry, SQLite persistence, alerts, and the authoritative stream sequence
- `crates/notch-metrics`: `sysinfo` host/process-tree sampling and bounded in-memory history
- `crates/notch-ipc`: authenticated local socket/named-pipe transport
- `crates/notch-hook`: fail-open `llm-notch-hook` helper used by reviewed vendor hooks
- `src-tauri`: lifecycle, commands, stream delivery, window adapters, tray, shortcut, autostart, and single-instance handling
- `src/native` + `src/state`: typed Tauri client and renderer state bridge

The host stores `llm-notch.sqlite3` under the Tauri app-data directory. On Unix, its directory is set to `0700` and the database to `0600`. Tests use temporary or in-memory databases.

The application performs no cloud calls and has no HTTP, shell, filesystem, process, or opener guest permissions.

Atomic bootstrap includes at most 256 recent events (with one reserved recent event per active/unresolved session). Older events are loaded deliberately through a bounded, cursor-based per-session API with pages of at most 100 events.

## Development

Requirements: a current Rust toolchain, Node.js/npm, and the platform prerequisites from the Tauri 2 documentation.

```bash
npm install
npm run native:dev
```

`native:dev` builds a target-specific debug helper first, then starts `tauri dev`. For browser-only marketing preview:

```bash
npm run dev
```

Useful verification commands:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
npm run typecheck
npm run lint
npm run test:run
npm run build
npm run native:check
```

## Helper packaging

Tauri bundles `llm-notch-hook` as an external binary. The deterministic preparation script builds the helper and copies it to Tauri's required target-suffixed path:

```bash
# Host release target
npm run native:prepare-helper

# Host debug target
npm run native:prepare-helper -- --debug

# Explicit cross target (toolchain/SDK must already be installed)
npm run native:prepare-helper -- --target x86_64-pc-windows-msvc
```

`tauri build` runs the release helper preparation automatically. The helper is not launched through a guest shell API; reviewed vendor hooks invoke its installed absolute path.

## IPC security and lifecycle

At each app start, the host creates a new 256-bit token and writes a runtime descriptor in the user's local app-data runtime directory. The token is never passed in argv, environment variables, logs, or SQLite.

- Unix: `0700` runtime directory, `0600` descriptor/socket, token authentication, and same-effective-UID peer verification
- Windows: token authentication and a current-user named-pipe security descriptor; inherited filesystem ACLs for the descriptor are reported as unverified rather than claimed as user-only
- Framing and fields are bounded; raw prompts, command bodies, tool input, and tool output are not forwarded or retained
- ACK is sent only after the host core accepts and persists an event; transient vendor delivery failures enter a bounded, time-ordered spool and replay files are removed only after acceptance
- Shutdown removes the owned descriptor and Unix socket

## Tauri capabilities

Capabilities are window-scoped custom-command permissions. There is no `core:default` and no broad plugin permission.

Overlay permissions:

- bootstrap and subscribe/unsubscribe
- open dashboard
- set overlay mode
- acknowledge local attention

Dashboard permissions add:

- settings, real monitor enumeration, startup, and shortcut commands
- metrics history and purge
- integration health and read-only template preview
- connector apply/remove commands currently return an explicit `not available` error; automatic vendor config writes are intentionally disabled

Acknowledgement only clears llm_notch's local attention state. It never approves, denies, or answers a vendor prompt.

## Metrics scope and limitations

Sampling runs off the UI thread through the metrics engine's cadence-aware `tick` path (1 second with active validated roots, 5 seconds while idle; a larger configured interval slows it further). Shipped vendor templates provide no trusted PID/start-time pair, so their attribution starts as `unknown`. Generic attribution becomes `exact` only while an explicit `(pid, processStartedAtMs)` pair matches the live process table.

The dashboard uses bounded live history for 15 minutes and SQLite buckets for 1-hour/24-hour views. Host, aggregate, and every session ID are separate series. Each persisted series is independently capped/downsampled to 720 representative points while preserving its first/last available timestamps; the UI plots against the fixed requested time domain and reports coverage/downsampling. The configured 1/6/24/72/168-hour retention value drives repository pruning and disables impossible ranges; manual purge clears buckets plus persisted and in-memory latest metrics.

Current limitations:

- process-tree CPU/RSS and I/O are best effort after a validated root; explicit vendor roots may be labeled `shared` or `heuristic`
- CPU needs a warm-up interval
- historical schema v2 does not persist quality metadata, so reloaded buckets use `unknown` attribution and unavailable I/O quality
- GPU, network throughput, energy/power, token counts, cost, and model progress are not measured
- Windows I/O may represent all process I/O rather than disk-only activity

## Integration setup

Nothing under `integrations/` is installed automatically. Use the dashboard's read-only preview, then follow [the manual installation guide](docs/integrations/installation.md) and review the exact template diff. Wrappers are fail-open and emit neutral output so monitoring cannot block an agent workflow.

Capability claims are documented in [the integration matrix](docs/integrations/capability-matrix.md). Protocol v1 is observation-only: `decisionResponse` and `contextOpen` are false for the bundled vendor templates.

## Platform status

- macOS is the primary development target. The overlay uses AppKit non-activating `NSWindow` styles and native `NSScreen.safeAreaInsets`. Accessory/regular activation-policy switching is attempted but reported as partial because AppKit can reject it outside a bundled app. Tauri creates an `NSWindow`, so true `NSPanel` semantics remain a documented fallback.
- Windows code retains `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`, taskbar exclusion, and topmost flags. Presentation above fullscreen apps is explicitly unsupported on Windows: the UI disables that preference and the backend rejects attempts to enable it. The MSVC Rust target and non-SQLite crates cross-check on macOS; the full desktop cross-check still requires a Windows SDK/CRT C toolchain for bundled SQLite, and runtime behavior must be validated on Windows.
- Other desktop targets use Tauri window behavior without native overlay enhancements.

## Release signing

Local builds are unsigned. A distributable macOS release requires a Developer ID certificate, hardened runtime/entitlements review, signing of the app and embedded helper, notarization, and stapling. Windows distribution requires an Authenticode certificate and signing of both the executable/installer and helper.

This repository does not claim that an installer is signed, notarized, or production-ready unless those release steps were actually performed.
