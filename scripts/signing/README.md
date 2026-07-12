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

1. `npm run native:prepare-helper` — sidecars copied to `src-tauri/binaries/llm-notch-hook-<target>` and `llm-notch-relay-<target>`
2. `npm run native:prepare-relay -- --target <triple>` — optional cross-compiled relay sidecars for SSH remote deploy (built unsigned in CI matrix; not Authenticode/codesign verified)
3. `npm run native:build` — Tauri bundles app + `externalBin` hook and relay
4. **Windows**: `scripts/signing/sign-windows.ps1` — Authenticode on `.exe`, `.msi`, and embedded `llm-notch-hook.exe` (relay signing not yet wired)
5. **macOS**: codesign app + helper, then `scripts/signing/notarize-macos.sh` (relay signing not yet wired)
6. Manual smoke: overlay does not steal focus; helper health probe passes; remote backend reports relay present when bundled

## Local development

Unsigned debug builds are expected. Use `LLM_NOTCH_HOOK_BIN` to point connectors at `target/debug/llm-notch-hook`. Use `LLM_NOTCH_RELAY_BIN` to point the remote registry at `target/debug/llm-notch-relay`.

See also [`docs/platform/release-gates.md`](../../docs/platform/release-gates.md).
