# Lane 6 handoff — Context navigation

**Branch:** `feat/lane-6-context`  
**Base:** `25d056a`  
**Status:** `LANE_6_COMPLETE`

## Summary

Implemented user-initiated context navigation in the Tauri host:

- Opaque validated `ln1_` context locators (encode/decode/parse)
- Tier selection capped by frozen `ContextOpenTier` and host allowlist
- Platform activation for macOS and Windows
- `open_session` returns `{ contextOpenTier, activated, message? }`
- Security documentation for locator policy

## New modules

| Path | Purpose |
|------|---------|
| `src-tauri/src/context/locator.rs` | Opaque locator wire tokens + validation |
| `src-tauri/src/context/resolve.rs` | Session → locator via process ancestry |
| `src-tauri/src/context/tier.rs` | Achievable tier + adapter cap + fallback copy |
| `src-tauri/src/context/activate.rs` | Activation dispatch |
| `src-tauri/src/context/platform/macos.rs` | NSWorkspace bundle activation |
| `src-tauri/src/context/platform/windows.rs` | EnumWindows + SetForegroundWindow |
| `src-tauri/src/context/platform/stub.rs` | Unsupported platform honest fallback |
| `src-tauri/src/commands/context.rs` | `open_session` Tauri command + `OpenSessionResult` |

## Command contract

```rust
open_session(session_id: String) -> OpenSessionResult

OpenSessionResult {
    context_open_tier: ContextOpenTier, // achieved tier
    activated: bool,
    message?: string,                   // safe fallback microcopy
}
```

Moved from `commands/overlay.rs` (was `NotAvailable` stub).

## First-release targets (honest)

| Host | macOS | Windows | Max tier claimed |
|------|-------|---------|------------------|
| Terminal.app | best-effort activate | n/a | `windowFocus` |
| iTerm2 | best-effort activate | n/a | `windowFocus` (exact only when pane verified) |
| Windows Terminal | n/a | HWND focus | `windowFocus` |
| VS Code | bundle activate | HWND focus | `appActivate` |
| Cursor | bundle activate | HWND focus | `appActivate` |

`exactPane` is never claimed on Windows. macOS exact pane requires verified pane hint; otherwise downgrades with message.

## Security

See [docs/integrations/context-security.md](../integrations/context-security.md).

Rejected inputs: path traversal, shell metacharacters, invalid encoding, overlong tokens.

## Cargo.toml touches

`src-tauri/Cargo.toml` adds:

```toml
base64.workspace = true
sysinfo.workspace = true
```

## Tests

```bash
cargo test -p llm-notch-desktop --lib
```

**Results:** 54 tests passing (2026-07-11), including:

- Locator validation (path escape, shell injection, round-trip)
- Tier capping and fallback messaging
- Process-name → host mapping
- Windows: never claims `exactPane`
- `open_session` result JSON camelCase serialization
- Platform activation smoke (current PID)

## Blockers / follow-ups for other lanes

| Item | Owner | Notes |
|------|-------|-------|
| Adapter `contextOpen` / `contextOpenTier` advertising | Adapter lanes | Bundled templates still `false`/`none`; UI button hidden until caps updated |
| Trusted `process_root` from vendor hooks | Hook/platform lane | Navigation needs live PID+start attribution |
| Renderer `openSession` return type | UI lane (optional) | TS client still `Promise<void>`; backend now returns `OpenSessionResult` |
| macOS `exactPane` verification | Platform lane | Needs Accessibility/tab correlation beyond bundle activate |
| Lane 10 manifest merge | Integration QA | `lib.rs` command registration already wired in this lane |

## Local dev note

Tauri build requires helper sidecar:

```powershell
cargo build -p notch-hook
Copy-Item target\debug\llm-notch-hook.exe `
  src-tauri\binaries\llm-notch-hook-x86_64-pc-windows-msvc.exe
```

No protocol freeze changes. No push/PR from this lane.
