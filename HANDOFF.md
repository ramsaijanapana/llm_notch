# Lane 9 — Platform and release HANDOFF

**Branch:** `feat/lane-9-platform`  
**Base:** `25d056a`  
**Worktree:** `llm_notch-wt-platform`

## Summary

Lane 9 delivers native overlay platform behavior, bundled sidecar path resolution, CI workflows, and release signing scaffolds. Windows overlay is a topmost, non-activating tool window with `WM_MOUSEACTIVATE → MA_NOACTIVATE`. macOS uses the strongest available post-construction AppKit configuration with honest partial capability reporting.

## Delivered

| Area | Path | Notes |
|------|------|-------|
| Windows overlay | `src-tauri/src/window/windows.rs` | `WS_EX_NOACTIVATE`, topmost, subclass hook |
| macOS overlay | `src-tauri/src/window/macos.rs` | Style-mask panel emulation; `FullScreenAuxiliary` best-effort |
| Helper path | `src-tauri/src/runtime/helper_path.rs` | `externalBin` + env + dev fallback |
| CI | `.github/workflows/ci.yml` | Windows/macOS Rust, frontend, Playwright, Tauri smoke |
| Signing scaffold | `scripts/signing/*` | Authenticode + notarization gates (no secrets) |
| Platform docs | `docs/platform/*` | Release gates, overlay honesty |
| Native tests | `src-tauri/tests/native_windows.rs` | DPI + style + helper path smoke |

## Tests run (local)

| Command | Result |
|---------|--------|
| `cargo test -p llm-notch-desktop --test native_windows` | **4 passed** |
| `cargo test -p llm-notch-desktop --lib window::windows` | **4 passed** |
| `cargo test -p llm-notch-desktop --lib runtime::helper_path` | **2 passed** |
| `npm run typecheck` | **pass** |
| `npm run test:run` | **152 passed** |
| `npm run lint` | pre-existing CRLF format drift (206 biome format warnings on untouched files) |

Record CI results after merge to main.

## Capability matrix (honest)

| Capability | Windows | macOS |
|------------|---------|-------|
| Non-activating overlay | Supported | Partial (NSWindow, not true NSPanel) |
| Topmost | Supported | Supported |
| Fullscreen overlay | Unsupported (pref reset) | Partial (`FullScreenAuxiliary` best-effort) |
| Per-monitor DPI | Validated at setup | Via Tauri monitor APIs |
| Bundled helper | `externalBin` → runtime resolver | Same |

## Blockers

| Blocker | Impact | Owner action |
|---------|--------|--------------|
| Signing secrets not in repo | Release builds remain unsigned | Add `WINDOWS_CERTIFICATE_*`, `APPLE_*` to GitHub Actions secrets |
| True macOS `NSPanel` | Stronger non-activation | Requires Tauri upstream or custom window factory |
| Git not on PATH in some shells | Use full path `C:\Program Files\Git\bin\git.exe` | Environment setup |

## Out of scope (do not touch in this lane)

- Connector merge logic, adapters, decision broker
- React onboarding in other worktrees
- Claiming guaranteed fullscreen overlay on any platform

## Integration points for other lanes

- Bootstrap logs `helper` / `helper_exists` at startup (`lib.rs`)
- `integration_health` uses shared `resolve_helper_path`
- Overlay capability telemetry via `setup_overlay` → `OverlayPlatformCapability`

## Next steps (post-merge)

1. Wire release workflow with signing secrets when certificates exist
2. Optional: macOS native integration test requiring GUI session
3. Monitor focus-steal reports on real hardware matrices
