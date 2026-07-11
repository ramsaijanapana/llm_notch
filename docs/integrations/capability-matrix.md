# Integration capability matrix (V1)

Honest comparison of what each adapter template can observe in protocol v1. **Decision response is false for all vendors** — llm_notch does not approve, deny, or answer on behalf of the agent.

## Summary table

| Capability | Cursor | Claude Code | Codex (hooks) | Codex (notify) | Generic emit |
|------------|--------|-------------|---------------|----------------|--------------|
| `events` | ✅ partial | ✅ partial | ✅ partial | ⚠️ minimal | ✅ if you emit |
| `attention` | ❌ none | ⚠️ partial | ⚠️ partial | ❌ none | ✅ explicit events only |
| `decisionResponse` | ❌ | ❌ | ❌ | ❌ | ❌ |
| `contextOpen` | ❌ | ❌ | ❌ | ❌ | ❌ |
| `processAttribution` | ❌ unknown | ❌ unknown | ❌ unknown | ❌ unknown | ✅ only with validated PID + start time |
| Install trust | hooks.json | settings.json | `/hooks` trust | config.toml | manual CLI |
| Fail-open hooks | ✅ wrapper | ✅ wrapper | ✅ wrapper | ✅ wrapper | N/A (explicit emit) |

Legend: ✅ reliable · ⚠️ partial/heuristic · ❌ not available in V1 template

## `AdapterCapabilities` wire values

```typescript
type AdapterCapabilities = {
  source: 'cursor' | 'claudeCode' | 'codex' | 'generic'
  events: boolean
  attention: 'full' | 'partial' | 'none'
  decisionResponse: boolean
  contextOpen: boolean
  processAttribution: 'exact' | 'shared' | 'heuristic' | 'unknown'
}
```

### Cursor (template defaults)

```json
{
  "source": "cursor",
  "events": true,
  "attention": "none",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown"
}
```

The shipped Cursor template has no validated process identity, so attribution is unavailable. Ordinary `preToolUse` is recorded as tool activity and never inferred as permission attention.

### Claude Code (template defaults)

```json
{
  "source": "claudeCode",
  "events": true,
  "attention": "partial",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown"
}
```

**Observation-only:** `PermissionRequest` is hooked for attention state, but the template never returns `permissionDecision` or blocks tools.

### Codex lifecycle hooks (template defaults)

```json
{
  "source": "codex",
  "events": true,
  "attention": "partial",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown"
}
```

**Beta / trust-gated:** Hooks do not run until reviewed in Codex `/hooks`. Enable via `features.hooks` (current); `features.codex_hooks` is deprecated.

**Observation-only:** `PermissionRequest` is hooked for attention state, but the template never returns `decision.behavior` allow/deny responses.

**PreToolUse limits:** Codex documentation notes not every tool path is interceptable.

### Codex legacy notify (fallback)

```json
{
  "source": "codex",
  "events": false,
  "attention": "none",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown"
}
```

Turn-completion signal only. Deprecated upstream.

### Generic emit

```json
{
  "source": "generic",
  "events": true,
  "attention": "full",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown"
}
```

The capability starts at `unknown`. It changes to `exact` only while the host has validated a live `(pid, processStartedAtMs)` pair. Explicit attention events are observation-only. Missing or mismatched identities are rejected or fall back to `unknown`.

## UI behavior tied to capabilities

| UI surface | Gated by |
|------------|----------|
| Session timeline | `events` |
| Attention badge / alert | `attention != none` |
| Approve / deny buttons | `decisionResponse` (always hidden in V1) |
| “Open in agent” | `contextOpen` (always hidden in V1) |
| Per-session CPU/RSS chart | `processAttribution != unknown` |
| Shared/heuristic badge | `processAttribution` quality metadata |

## What V1 explicitly does not claim

- Remote approval of Cursor, Claude, or Codex permission dialogs
- GPU, network, or energy metrics
- Token counts, cost, or progress bars
- Opening the agent UI at the exact file/line from hook payloads
- Mac App Store sandboxed hook installation
