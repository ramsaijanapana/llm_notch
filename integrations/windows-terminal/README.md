# Windows Terminal collector (honest tab/pane metadata)

LLM Notch can navigate back to an agent session in Windows Terminal when hooks supply **verified** terminal metadata. This integration documents what Windows Terminal actually exposes and how to wire a collector without inventing IDs.

## What Windows Terminal provides

| Source | Variable | Auto-discovered? | ExactPane use |
|--------|----------|------------------|---------------|
| Shell integration | `WT_SESSION` | Yes (per tab/pane GUID) | Stored as `terminal_session_id`; **cannot** target `wt.exe focus-tab` by GUID today |
| Shell integration | `WT_PROFILE_ID` | Yes | Informational only |
| Shell integration | `WT_PROFILE_NAME` | Yes | Informational only |
| Shell integration | Tab index | **No env var** | Not available from WT APIs |
| Shell integration | Pane index | **No env var** | Not available from WT APIs |
| User / layout config | `LLM_NOTCH_TAB_ID` | Only if you set it | `wt.exe focus-tab -t` |
| User / layout config | `LLM_NOTCH_PANE_ID` | Only if you set it | `wt.exe focus-pane -t` |
| Platform collector | `LLM_NOTCH_WINDOW_HANDLE` | When `llm-notch-hook collect-terminal-env` or hook ingest discovers a verified HWND | HWND window-focus fallback |

**Limitation:** Windows Terminal does not publish tab or pane numeric indices through environment variables or a supported query API. `WT_SESSION` identifies the connection but `wt.exe` cannot focus a tab by session GUID yet ([terminal#19783](https://github.com/microsoft/terminal/issues/19783)). LLM Notch therefore:

1. Always passes through `WT_SESSION` when present.
2. Passes through `LLM_NOTCH_TAB_ID` / `LLM_NOTCH_PANE_ID` only when already set or explicitly configured by you.
3. Never parses window titles or guesses indices from HWND enumeration.
4. May discover `LLM_NOTCH_WINDOW_HANDLE` by walking the process tree to a classified terminal host and validating the HWND with Win32 (`hwnd_collector.rs`).

When only `WT_SESSION` is available, context open downgrades to window-focus or reports that exact-pane routing is incomplete.

## Components

| Path | Role |
|------|------|
| `integrations/wrappers/llm-notch-wt-collector.ps1` | PowerShell collector for profiles and hook wrappers |
| `integrations/wrappers/llm-notch-hook-wrapper.ps1` | Dot-sources the WT collector when present |
| `crates/notch-platform/src/wt_collector.rs` | Rust env reader + export helpers (unit tested) |
| `crates/notch-platform/src/hwnd_collector.rs` | Verified HWND discovery via process-tree walk + `IsWindow` |
| `crates/notch-ipc/src/collector.rs` | Hook ingest enrichment from `LLM_NOTCH_*` / `WT_SESSION` |

## Quick setup

### 1. Enable Windows Terminal shell integration

In Windows Terminal **Settings → Defaults → Interaction**, enable shell integration for your profile. Confirm in a new tab:

```powershell
$env:WT_SESSION
```

You should see a GUID. If empty, the collector cannot discover a session id (common when WT is the default terminal handler for shortcuts that bypass shell integration).

### 2. Load the collector in your shell profile

Add to `$PROFILE` (or your WT profile `commandline`):

```powershell
. "$HOME\.cursor\hooks\llm-notch-wt-collector.ps1"
Export-LlmNotchWtCollectorEnv
```

Or reference the repo copy during development:

```powershell
. "C:\dev\llm_notch\integrations\wrappers\llm-notch-wt-collector.ps1"
Export-LlmNotchWtCollectorEnv
```

### 3. (Optional) Supply tab/pane indices for fixed layouts

Only when you control tab order (for example a startup batch that always opens the same layout):

```powershell
Export-LlmNotchWtCollectorEnv -TabId '1' -PaneId '0'
```

These values are **your** layout declaration, not auto-discovered from Windows Terminal.

### 4. Wire hooks through the PowerShell wrapper

The hook wrapper automatically dot-sources `llm-notch-wt-collector.ps1` from the same directory before invoking `llm-notch-hook`:

```text
pwsh -NoProfile -File "%USERPROFILE%\.cursor\hooks\llm-notch-hook-wrapper.ps1" -Source cursor -VendorEvent sessionStart
```

Copy both scripts together:

```
%USERPROFILE%\.cursor\hooks\
  llm-notch-hook-wrapper.ps1
  llm-notch-wt-collector.ps1
```

## Environment variables consumed by hooks

| Variable | Set by collector when |
|----------|----------------------|
| `LLM_NOTCH_TERMINAL_SESSION_ID` | `WT_SESSION` or explicit override present |
| `LLM_NOTCH_TAB_ID` | Already in env or you passed `-TabId` |
| `LLM_NOTCH_PANE_ID` | Already in env or you passed `-PaneId` |
| `LLM_NOTCH_WINDOW_HANDLE` | Verified HWND from env, `collect-terminal-env`, or hook ingest discovery |

`llm-notch-hook` reads these via `notch-ipc` collector enrichment and emits `verified_terminal` on the session when any field is present.

## Exact-pane routing honesty

Full ExactPane activation requires `terminal_session_id` + `tab_id` + `pane_id` (see `notch-platform` `resolve_tier`). With a `WT_SESSION` GUID:

- `build_wt_exact_pane_command` rejects GUID session ids for `wt.exe -w` targeting.
- Activation falls back to HWND window focus when `LLM_NOTCH_WINDOW_HANDLE` is set, otherwise reports unavailable.

For reliable exact-pane today, supply numeric `LLM_NOTCH_TAB_ID` and `LLM_NOTCH_PANE_ID` from a layout you control **and** use a numeric window index (`0`, `1`, …) or name for `LLM_NOTCH_TERMINAL_SESSION_ID` instead of the `WT_SESSION` GUID when calling `wt.exe -w`.

## Validation

Rust unit tests:

```powershell
cargo test -p notch-platform hwnd_collector wt_collector
```

PowerShell smoke test (mock env):

```powershell
$env:WT_SESSION = '5720ee6d-6474-47b0-88db-fa7e10e60d37'
$env:LLM_NOTCH_TAB_ID = '1'
$env:LLM_NOTCH_PANE_ID = '0'
. .\integrations\wrappers\llm-notch-wt-collector.ps1
Export-LlmNotchWtCollectorEnv | ConvertTo-Json
```

Expected: `terminalSessionId` mirrors `WT_SESSION`; tab/pane pass through unchanged.
