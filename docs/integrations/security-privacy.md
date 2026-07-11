# Security and privacy

How llm_notch handles credentials, raw vendor payloads, and local threat boundaries.

## Authentication model

| Asset | Storage | Exposure |
|-------|---------|----------|
| IPC bearer token | Runtime descriptor file, user-only permissions | Never in hook configs, argv, env, logs, or SQLite |
| Descriptor path | Discovered by signed helper | Not hard-coded in templates |
| Vendor hook JSON | stdin → normalize → discard | Not persisted by default |

Hooks call the installed `llm-notch-hook` binary. Release distributions must sign it; local development binaries are unsigned. The helper reads the descriptor and attaches auth to IPC frames. Wrapper scripts never print or export tokens.

### Runtime descriptor

| Platform | Directory (typical) | Files |
|----------|---------------------|-------|
| macOS | User-local app data from `directories::ProjectDirs` | Unix socket + descriptor |
| Windows | User-local app data from `directories::ProjectDirs` | named pipe + descriptor |

Descriptor permissions:

- macOS: file `0600`, directory `0700`
- Windows: the named pipe has a current-user security descriptor; descriptor-file ACL inheritance is not yet independently verified

Token rotates every app start. Quit removes the descriptor.

## Threat boundary (honest)

**In scope:** Prevent casual cross-user IPC, accidental secret leakage in logs/UI, and hook scripts blocking agents.

**Out of scope:** Same-user malware with code execution. A malicious process running as the user can read the descriptor while the app is running. This is accepted for V1 local-first tooling.

Peer verification:

- Unix: effective UID from local-socket peer credentials
- Windows: named-pipe security descriptor plus token authentication; PID-only peer capability is reported honestly

If peer verification is unavailable on a platform build, capability flags document the gap.

## Raw payload redaction

The normalizer **must not** retain:

| Vendor field | Handling |
|--------------|----------|
| Full prompts / user messages | Dropped |
| Shell command bodies | Dropped after permission classification |
| Tool stdout/stderr | Dropped |
| File contents from Read/Write | Dropped |
| API keys, tokens, passwords | Scrubbed if accidentally present; event rejected if unrecoverable |
| Absolute home paths | May collapse to workspace-relative labels |

### What is kept (bounded)

| Field | Max size |
|-------|----------|
| `summary` | 512 bytes (`MAX_EVENT_SUMMARY_LEN`) |
| `label` / `workspaceLabel` | 256 bytes each |
| `externalSessionId` | 256 bytes |
| `toolName` | 128 bytes |

Summaries are human-readable redacted descriptions, e.g.:

- ✅ `"Shell tool requested (redacted)"`
- ✅ `"Permission dialog observed"`
- ❌ `"curl -H 'Authorization: Bearer sk-...' https://..."`

## Hook script safety

Templates follow Cursor hook skill principles:

1. **Fail open** — exit `0`, return `{}` for observation-only paths
2. **No network** — wrappers do not curl or phone home
3. **No secret env vars** — `LLM_NOTCH_HOOK_BIN` is path override only
4. **Short timeout** — default 2s; wrapper kills slow helper
5. **No shell injection** — helper invoked as argv array; vendor JSON via stdin file descriptor

Wrappers intentionally avoid `jq`, `python`, and `node` so hook environments stay minimal.

## Installer security (future dashboard flow)

Before writing any config:

1. Hash target file; abort if changed since preview
2. Reject symlink escapes and paths outside allowed scopes
3. Write timestamped backup beside target
4. Merge only llm_notch entries; preserve unrelated hooks
5. Show unified diff to the user; require explicit confirm

Nothing installs silently on first app launch. Connect/apply runs only after explicit dashboard or onboarding confirmation.

Dashboard decision responses are delivered to Claude Code hooks when capabilities allow; observation-only vendors remain fail-open.

## Spool directory (helper offline mode)

When the desktop host is not running:

- Atomic JSON files in user spool dir
- Cap: 1000 files / 10 MiB total
- Oldest dropped first
- No spool file contains raw vendor payloads after normalization

## Telemetry logging

- Rust `tracing` in helper/host must not log stdin bodies or tokens
- Diagnostic logs contain event classes/request metadata only, never bodies or tokens

## User actions in V1

| Action | Effect |
|--------|--------|
| Acknowledge attention in overlay | Local UI state only |
| Open dashboard | Focus llm_notch window |
| Allow/deny in-app decision (Claude Code) | Delivered to hook when capability + transport succeed; otherwise failed/expired |
| Click “open in agent” | Hidden — `contextOpen: false` |

Observation-only vendors (Cursor, Codex) never receive in-app allow/deny from llm_notch V1 templates.
