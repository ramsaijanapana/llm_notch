# Antigravity CLI integration (connector scaffold)

Antigravity CLI stores workspace hooks in `.agents/hooks.json`. The file uses a **named-hook wrapper** (each top-level key is a hook group) rather than a flat `hooks` object.

[Antigravity documents](https://antigravity.google/docs/hooks) `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, and `Stop`. Hook stdin uses camelCase fields such as `conversationId` and `workspacePaths`, which differ from the Claude/Gemini hook contracts already mapped by `llm-notch-hook`.

This repository ships:

- A merge-safe template fragment under `hooks.json.template`
- Fixture payloads under `integrations/fixtures/antigravity-cli/`
- Connector merge/remove tests in `notch-connectors`

The agent is **verified in catalog** with hook stdin mapping for `conversationId`, `workspacePaths`, and nested `toolCall.name`. Live end-to-end verification on real Antigravity installs is still recommended.
