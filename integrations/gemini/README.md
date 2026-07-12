# Gemini CLI integration

LLM Notch merges the bundled hook fragment into `~/.gemini/settings.json` for user scope or `.gemini/settings.json` for project scope. Existing settings and non-LLM-Notch hooks are preserved, and every changed file is backed up before an atomic write.

The connector observes `SessionStart`, `BeforeTool`, `AfterTool`, `Notification`, and `SessionEnd`. Gemini notification hooks are observation-only: permission alerts can be displayed in LLM Notch but not approved remotely.

Use the dashboard's Integrations screen to preview the exact diff before applying, repair, disable, or roll back the connector.
