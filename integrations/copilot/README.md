# GitHub Copilot CLI integration

LLM Notch merges the bundled hook fragment into `~/.copilot/hooks/llm-notch.json` for user scope or `.github/hooks/llm-notch.json` for project scope. Existing hook files and non-LLM-Notch entries are preserved, and every changed file is backed up before an atomic write.

[GitHub documents](https://docs.github.com/en/copilot/reference/hooks-reference) Copilot CLI lifecycle hooks with camelCase event names: `sessionStart`, `sessionEnd`, `preToolUse`, `postToolUse`, `permissionRequest`, and `agentStop`. Hook stdin delivers JSON payloads with `sessionId`, `cwd`, and event-specific fields.

The connector observes session lifecycle, tool activity, permission attention, and turn completion. Permission hooks are observation-only in V1: LLM Notch never returns `permissionDecision` or `behavior` responses to Copilot CLI.

Use the dashboard's Integrations screen to preview the exact diff before applying, repairing, disabling, or rolling back the connector.
