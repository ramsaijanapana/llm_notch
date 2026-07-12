# Integration capability matrix (V1)

Honest comparison of what each adapter template can observe in protocol v1. Decision responses are **capability-gated** — only shown when the detected vendor version supports a verified response path and the broker has actionable payload.

## Summary table

| Capability | Cursor | Claude Code | Codex (hooks) | Codex (notify) | Gemini CLI | Qwen Code | Copilot CLI | Generic emit |
|------------|--------|-------------|---------------|----------------|------------|-----------|-------------|--------------|
| `events` | ✅ partial | ✅ partial | ✅ partial | ⚠️ minimal | ✅ partial | ✅ partial | ✅ partial | ✅ if you emit |
| `attention` | ❌ none | ⚠️ partial | ⚠️ partial | ❌ none | ⚠️ partial | ⚠️ partial | ⚠️ partial | ✅ explicit events only |
| `decisionResponse` | ❌ | ⚠️ gated | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `contextOpen` | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `processAttribution` | ❌ unknown | ❌ unknown | ❌ unknown | ❌ unknown | ❌ unknown | ❌ unknown | ❌ unknown | ✅ only with validated PID + start time |
| Install trust | hooks.json | settings.json | `/hooks` trust | config.toml | settings.json | settings.json | hooks/*.json | manual CLI |
| Fail-open hooks | ✅ wrapper | ✅ helper | ✅ wrapper | ✅ wrapper | ✅ helper | ✅ helper | ✅ helper | N/A (explicit emit) |

Legend: ✅ reliable · ⚠️ partial/heuristic · ❌ not available in V1 template

## `AdapterCapabilities` wire values

```typescript
type AdapterCapabilities = {
  source: 'cursor' | 'claudeCode' | 'codex' | 'gemini' | 'generic'
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

Known Claude Code version (≥ 2.1.0):

```json
{
  "source": "claudeCode",
  "events": true,
  "attention": "partial",
  "decisionResponse": true,
  "contextOpen": false,
  "processAttribution": "unknown",
  "respondDecisions": true,
  "respondQuestions": false,
  "failOpenHooks": true,
  "requiresExternalTrust": false
}
```

Unknown Claude Code version (observation-only):

```json
{
  "source": "claudeCode",
  "events": true,
  "attention": "partial",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown",
  "respondDecisions": false,
  "respondQuestions": false,
  "failOpenHooks": true,
  "requiresExternalTrust": false
}
```

**Verified response paths (known versions only):**

- `PermissionRequest`: `hookSpecificOutput.decision.behavior` allow/deny ([Claude Code hooks reference](https://code.claude.com/docs/en/hooks))
- `ExitPlanMode`: `PreToolUse` with `permissionDecision: "allow"` and required `updatedInput`

**Not shipped:** generic `AskUserQuestion` answer path.

**Fail-open default:** templates and helper emit `{}` until the broker returns a verified decision response.

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

### Gemini CLI (template defaults)

```json
{
  "source": "gemini",
  "events": true,
  "attention": "partial",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown",
  "respondDecisions": false,
  "respondQuestions": false,
  "failOpenHooks": true,
  "requiresExternalTrust": false
}
```

**Observation-only:** Shipped hooks cover `SessionStart`, `BeforeTool`, `AfterTool`, `Notification`, and `SessionEnd`. `Notification` (including `ToolPermission`) surfaces attention in LLM Notch but never returns allow/deny decisions to Gemini CLI.

### Qwen Code (template defaults)

```json
{
  "source": "claudeCode",
  "events": true,
  "attention": "partial",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown",
  "respondDecisions": false,
  "respondQuestions": false,
  "failOpenHooks": true,
  "requiresExternalTrust": false
}
```

**Claude-compatible wire:** Qwen Code documents the same stdin JSON contract as Claude Code. The shipped template uses the `claudeCode` helper discriminator until a distinct `AgentSource::Qwen` lands. Install paths remain Qwen-specific (`~/.qwen/settings.json`, `.qwen/settings.json`).

**Observation-only:** `PermissionRequest` surfaces attention but never returns `permissionDecision` responses.

### Copilot CLI (template defaults)

```json
{
  "source": "generic",
  "events": true,
  "attention": "partial",
  "decisionResponse": false,
  "contextOpen": false,
  "processAttribution": "unknown",
  "respondDecisions": false,
  "respondQuestions": false,
  "failOpenHooks": true,
  "requiresExternalTrust": false
}
```

**Observation-only:** Shipped hooks cover `sessionStart`, `preToolUse`, `postToolUse`, `permissionRequest`, `agentStop`, and `sessionEnd`. The helper uses the `copilotCli` wire discriminator and maps camelCase stdin (`sessionId`, `toolName`) from [GitHub's hooks reference](https://docs.github.com/en/copilot/reference/hooks-reference). Permission hooks never return `behavior` or `permissionDecision` responses.

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
| Approve / deny buttons | `decisionResponse` and verified vendor response path |
| “Open in agent” | `contextOpen` (always hidden in V1) |
| Per-session CPU/RSS chart | `processAttribution != unknown` |
| Shared/heuristic badge | `processAttribution` quality metadata |

## What V1 explicitly does not claim

- Remote approval on unknown vendor versions or unverified hook events
- Generic question answering for Claude Code `AskUserQuestion`
- GPU, network, or energy metrics
- Token counts, cost, or progress bars
- Opening the agent UI at the exact file/line from hook payloads
- Mac App Store sandboxed hook installation
