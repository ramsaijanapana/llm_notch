# Context navigation security

Lane 6 implements user-initiated context open for attributed agent sessions. This document describes locator safety, tier honesty, and platform boundaries.

## Threat model

Context navigation is **user-initiated only**. Unlike the non-activating overlay, `open_session` may foreground another application when the user explicitly requests it from the dashboard or overlay.

Out of scope:

- Arbitrary path or URL open from renderer input
- Shell command execution with user-supplied strings
- Recursive filesystem scans to find agent windows
- Background or ambient focus stealing

## Opaque locators

Locators are opaque `ln1_` tokens produced and consumed only in the Rust host:

| Property | Policy |
|----------|--------|
| Wire prefix | `ln1_` (version 1) |
| Encoding | URL-safe base64 of JSON payload |
| Max length | 512 bytes |
| Renderer exposure | Token only; structured payload never crosses IPC |

### Payload fields (host-internal)

- `host`: allowlisted terminal/editor kind
- `pid` + `started_at_ms`: optional validated `ProcessIdentity`
- `pane_hint`: optional bounded label (workspace label), never a filesystem path

### Rejected locator input

The host rejects locators that contain:

- Path traversal (`..`, `/`, `\` in pane hints)
- Shell metacharacters (`;`, `|`, `` ` ``, `$`, `&`, redirection, subshells)
- Invalid prefix, encoding, or version
- Overlong tokens or pane hints (>64 chars)

Locators are validated on both encode and parse. The renderer never supplies raw locator strings in V1; the host derives them from session attribution.

## Tier honesty (`ContextOpenTier`)

Frozen wire enum: `none | appActivate | windowFocus | exactPane`.

| Tier | Meaning | First-release targets |
|------|---------|----------------------|
| `none` | No navigation performed | Unsupported adapter, missing attribution, platform failure |
| `appActivate` | Bring host application forward | VS Code, Cursor |
| `windowFocus` | Focus a top-level host window | Windows Terminal; Terminal.app / iTerm2 best-effort |
| `exactPane` | Verified tab/pane/editor group | **Not claimed** unless verification succeeds |

Rules:

1. Adapter capability caps the maximum tier (`AdapterCapabilities.contextOpenTier`).
2. Host kind limits achievable tier (editors do not claim `exactPane` in V1).
3. `open_session` returns the **achieved** tier, not the requested tier.
4. Fallback messaging explains downgrades (e.g. exact → window focus).

## Platform activation

### macOS

- `NSWorkspace` / bundle identifiers for Terminal.app, iTerm2, VS Code, Cursor
- Exact pane: best-effort only; downgrades to `windowFocus` without verified pane correlation

### Windows

- `EnumWindows` + `SetForegroundWindow` for attributed PID
- Exact pane: **never claimed**; downgrades to `windowFocus` or `appActivate`

### Other platforms

Return `none` with explicit unsupported message.

## Command contract

`open_session(sessionId)` returns:

```json
{
  "contextOpenTier": "appActivate",
  "activated": true,
  "message": "optional fallback explanation"
}
```

Errors:

- Invalid session id → `invalid request`
- Unknown session → `not found`

Successful responses may still report `activated: false` with honest `message` when navigation is unavailable.

## Process attribution dependency

Context navigation requires a live, validated `process_root` on the session for window-level tiers. Without attribution, the host returns `none` and instructs the user to open the agent manually.

Generic protocol clients may later supply richer locator hints; those must pass the same validation pipeline.

## Related documents

- [CONTRACT_FREEZE.md](../parity/CONTRACT_FREEZE.md) — frozen `ContextOpenTier` wire type
- [security-privacy.md](./security-privacy.md) — broader host security posture
- [capability-matrix.md](./capability-matrix.md) — adapter `contextOpen` advertising
