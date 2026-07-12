# Qwen Code integration

LLM Notch merges the bundled hook fragment into `~/.qwen/settings.json` for user scope or `.qwen/settings.json` for project scope. Existing settings and non-LLM-Notch hooks are preserved, and every changed file is backed up before an atomic write.

[Qwen Code documents](https://qwenlm.github.io/qwen-code-docs/en/users/features/hooks/) a Claude-compatible hook surface: `PreToolUse`, `PostToolUse`, `SessionStart`, `SessionEnd`, `PermissionRequest`, and `Stop`. The shipped template routes observation through the `claudeCode` helper wire discriminator because Qwen's stdin JSON contract matches Claude Code. Distinct Qwen install paths are still detected and merged by the connector adapter.

The connector observes session lifecycle, tool activity, permission attention, and turn completion. Permission hooks are observation-only in V1: LLM Notch never returns `permissionDecision` responses to Qwen Code.

Use the dashboard's Integrations screen to preview the exact diff before applying, repairing, disabling, or rolling back the connector.
