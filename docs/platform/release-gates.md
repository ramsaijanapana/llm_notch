# Platform release gates

Lane 9 defines the minimum gates before distributing signed desktop builds.

## Artifact pipeline

```mermaid
flowchart LR
  A[npm run build] --> B[native:prepare-helper]
  B --> C[src-tauri/binaries/llm-notch-hook-target]
  C --> D[tauri build]
  D --> E{Platform}
  E -->|Windows| F[Authenticode sign]
  E -->|macOS| G[codesign + notarize]
  F --> H[Release]
  G --> H
```

## Sidecar packaging (`externalBin`)

`src-tauri/tauri.conf.json` declares:

```json
"externalBin": ["binaries/llm-notch-hook"]
```

`npm run native:prepare-helper` copies the Cargo `notch-hook` binary to:

```
src-tauri/binaries/llm-notch-hook-<rust-target-triple>[.exe]
```

Tauri renames this to `llm-notch-hook` / `llm-notch-hook.exe` beside the main executable in the bundle. Runtime resolution lives in `src-tauri/src/runtime/helper_path.rs` and is logged at host startup.

## Windows overlay guarantees (validated in CI)

| Guarantee | Mechanism |
|-----------|-----------|
| Topmost HUD | `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)` |
| No taskbar button | `WS_EX_TOOLWINDOW`, `WS_EX_APPWINDOW` cleared |
| No focus steal | `WS_EX_NOACTIVATE` + `WM_MOUSEACTIVATE → MA_NOACTIVATE` subclass |
| Per-monitor DPI | Process must be per-monitor aware (V2 preferred); validated in `native_windows` tests |

`show_over_fullscreen` is **unsupported** on Windows; persisted preference is reset at startup.

## macOS overlay honesty

| Approach | Status |
|----------|--------|
| `NSWindow` + `NonactivatingPanel` style mask | Implemented (Tauri default construction path) |
| True `NSPanel` at construction | Not available without forking Tauri window creation |
| `FullScreenAuxiliary` | Best-effort only; not guaranteed over every fullscreen host |

See [`macos-overlay.md`](macos-overlay.md).

## Signing gates (secrets required)

| Gate | Script | Blocks release when |
|------|--------|---------------------|
| Authenticode | `scripts/signing/sign-windows.ps1` | Secrets absent or signature not `Valid` |
| Developer ID + notarization | `scripts/signing/notarize-macos.sh` | Helper missing, codesign verify fails, or notary rejects |

CI runs `release-gates` job to verify scaffold files exist; it does **not** sign.

## Local verification (unsigned)

```powershell
cargo test -p llm-notch-desktop --test native_windows
cargo test --workspace
npm run test:run
npx playwright test --project=chromium --project=webkit
```
