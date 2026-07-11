# macOS overlay platform notes

## Strongest reliable path: construction-time `NSPanel`

Apple's recommended pattern for a floating, non-activating HUD is an [`NSPanel`](https://developer.apple.com/documentation/appkit/nspanel) created with:

- `NSWindowStyleMask::NonactivatingPanel`
- `NSFloatingWindowLevel` (or higher when appropriate)
- `setBecomesKeyOnlyIfNeeded(true)` / `setWorksWhenModal(true)` as needed
- `NSWindowCollectionBehavior::CanJoinAllSpaces | IgnoresCycle | Stationary`
- Optional `FullScreenAuxiliary` when `show_over_fullscreen` is enabled

A true `NSPanel` refuses key-window status unless explicitly requested, which is stronger than applying style masks to a plain `NSWindow`.

## What Tauri 2 provides today

Tauri constructs overlay webviews as [`NSWindow`](https://developer.apple.com/documentation/appkit/nswindow) instances. Lane 9 applies panel-like configuration **after** creation in `src-tauri/src/window/macos.rs`:

- Style mask: `Borderless | NonactivatingPanel | UtilityWindow`
- Level: `NSFloatingWindowLevel`
- Collection behavior: `CanJoinAllSpaces`, `IgnoresCycle`, `Stationary`, optional `FullScreenAuxiliary`
- Activation policy: `NSApplicationActivationPolicy::Accessory` when dashboard is hidden

This is the strongest approach available **without** replacing Tauri's window factory. Capability reports mark `non_activating` and `activation_policy` as **partial** because AppKit may still promote focus in edge cases (modal sheets, secure input, accessibility tools).

## `FullScreenAuxiliary` — not guaranteed

`NSWindowCollectionBehavior::FullScreenAuxiliary` improves visibility beside some fullscreen applications. It does **not** guarantee coverage across:

- Every third-party fullscreen host
- All Spaces / Stage Manager transitions
- Screen Recording or secure-input prompts

Product copy must not claim guaranteed fullscreen overlay behavior. Windows does not implement an equivalent and resets `show_over_fullscreen` at startup.

## Future upgrade path (out of Lane 9 scope)

A construction-time `NSPanel` would require one of:

1. Upstream Tauri support for panel window kinds, or
2. A custom AppKit window factory integrated before webview attachment (high maintenance)

Until then, monitor focus-steal reports and keep the partial capability fallback in bootstrap telemetry.
