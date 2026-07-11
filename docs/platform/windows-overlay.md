# Windows overlay platform notes

## Non-activating topmost tool window

The overlay HWND is configured in `src-tauri/src/window/windows.rs`:

| Flag / call | Purpose |
|-------------|---------|
| `WS_EX_NOACTIVATE` | Prevent activation on show |
| `WS_EX_TOOLWINDOW` | Exclude from taskbar |
| `!WS_EX_APPWINDOW` | Avoid Alt+Tab listing |
| `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)` | Stay above normal windows without focus |
| Chained `GWLP_WNDPROC` → `MA_NOACTIVATE` on `WM_MOUSEACTIVATE` | Defense in depth when user clicks the HUD |

Together these ensure the overlay does not steal focus from the active application.

## Per-monitor DPI awareness

Mixed-DPI positioning depends on the process being per-monitor aware. The host validates awareness at overlay setup via `validate_per_monitor_dpi_awareness()` and logs a warning if only system-aware or unaware contexts are detected.

Smoke tests in `src-tauri/tests/native_windows.rs` assert the test/CI process meets the minimum bar. Tauri/WebView2 builds typically ship with Per-Monitor V2 in the application manifest.

## Unsupported settings

`show_over_fullscreen` has no Windows equivalent. `lib.rs` resets the persisted preference and logs a warning.
