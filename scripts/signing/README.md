# Release signing and notarization gates

Production releases **must not ship** until Authenticode (Windows) and Developer ID + notarization (macOS) succeed. CI builds unsigned artifacts by design; signing runs only in protected release workflows with repository secrets.

## Required repository secrets (never commit)

| Secret | Platform | Purpose |
|--------|----------|---------|
| `WINDOWS_CERTIFICATE_BASE64` | Windows | PFX / Authenticode cert (base64) |
| `WINDOWS_CERTIFICATE_PASSWORD` | Windows | PFX password |
| `APPLE_CERTIFICATE_BASE64` | macOS | Developer ID Application .p12 |
| `APPLE_CERTIFICATE_PASSWORD` | macOS | .p12 password |
| `APPLE_ID` | macOS | Notarization Apple ID |
| `APPLE_ID_PASSWORD` | macOS | App-specific password |
| `APPLE_TEAM_ID` | macOS | Team identifier |

## Gate checklist

1. `npm run native:prepare-helper` — sidecar copied to `src-tauri/binaries/llm-notch-hook-<target>`
2. `npm run native:build` — Tauri bundles app + `externalBin` helper
3. **Windows**: `scripts/signing/sign-windows.ps1` — Authenticode on `.exe`, `.msi`, and embedded `llm-notch-hook.exe`
4. **macOS**: codesign app + helper, then `scripts/signing/notarize-macos.sh`
5. Manual smoke: overlay does not steal focus; helper health probe passes

## Local development

Unsigned debug builds are expected. Use `LLM_NOTCH_HOOK_BIN` to point connectors at `target/debug/llm-notch-hook`.

See also [`docs/platform/release-gates.md`](../../docs/platform/release-gates.md).
