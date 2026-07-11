# Example backup and rollback

Walkthrough for manual Cursor user-scope install. Adapt paths for Claude Code or Codex.

## Before state

`~/.cursor/hooks.json`:

```json
{
  "version": 1,
  "hooks": {
    "beforeShellExecution": [
      {
        "command": "hooks/approve-network.sh",
        "matcher": "curl|wget",
        "failClosed": true
      }
    ]
  }
}
```

## Step 1 — Preview

```bash
diff -u ~/.cursor/hooks.json \
  <(sed 's|integrations/wrappers|hooks|g' integrations/cursor/hooks.json.template)
```

User confirms unrelated `beforeShellExecution` hook remains.

## Step 2 — Backup

```bash
STAMP=$(date +%Y%m%dT%H%M%S)
cp ~/.cursor/hooks.json ~/.cursor/hooks.json.llm-notch.bak.$STAMP
echo "Backup: ~/.cursor/hooks.json.llm-notch.bak.$STAMP"
```

Result:

```
Backup: ~/.cursor/hooks.json.llm-notch.bak.20260711T110300
```

## Step 3 — Install wrapper only

```bash
mkdir -p ~/.cursor/hooks
install -m 755 integrations/wrappers/llm-notch-hook-wrapper.sh ~/.cursor/hooks/
```

## Step 4 — Merge write (illustrative result)

`~/.cursor/hooks.json` after merge:

```json
{
  "version": 1,
  "hooks": {
    "beforeShellExecution": [
      {
        "command": "hooks/approve-network.sh",
        "matcher": "curl|wget",
        "failClosed": true
      }
    ],
    "sessionStart": [
      {
        "command": "hooks/llm-notch-hook-wrapper.sh --source cursor --vendor-event sessionStart",
        "timeout": 2
      }
    ],
    "preToolUse": [
      {
        "command": "hooks/llm-notch-hook-wrapper.sh --source cursor --vendor-event preToolUse",
        "timeout": 2
      }
    ],
    "postToolUse": [
      {
        "command": "hooks/llm-notch-hook-wrapper.sh --source cursor --vendor-event postToolUse",
        "timeout": 2
      }
    ],
    "postToolUseFailure": [
      {
        "command": "hooks/llm-notch-hook-wrapper.sh --source cursor --vendor-event postToolUseFailure",
        "timeout": 2
      }
    ],
    "stop": [
      {
        "command": "hooks/llm-notch-hook-wrapper.sh --source cursor --vendor-event stop",
        "timeout": 2
      }
    ],
    "sessionEnd": [
      {
        "command": "hooks/llm-notch-hook-wrapper.sh --source cursor --vendor-event sessionEnd",
        "timeout": 2
      }
    ]
  }
}
```

Note: `beforeShellExecution` with `failClosed: true` is unrelated and untouched.

## Step 5 — Verify JSON

```bash
./integrations/validate-json.sh
```

## Rollback scenario

User notices unexpected hook latency (not from llm_notch — wrapper times out at 2s max):

```bash
cp ~/.cursor/hooks.json.llm-notch.bak.20260711T110300 ~/.cursor/hooks.json
```

Verify restoration:

```bash
python3 -m json.tool ~/.cursor/hooks.json | head
# beforeShellExecution present, sessionStart absent
```

Restart Cursor.

## Partial rollback (remove llm_notch only)

If other hooks were added after install, manually delete llm_notch `command` entries containing `llm-notch-hook-wrapper` rather than restoring an old backup wholesale.

## Windows backup example

```powershell
$stamp = Get-Date -Format "yyyyMMddTHHmmss"
$target = Join-Path $env:USERPROFILE ".cursor\hooks.json"
$backup = "$target.llm-notch.bak.$stamp"
Copy-Item $target $backup
Write-Host "Backup: $backup"
```

Rollback:

```powershell
Copy-Item "$env:USERPROFILE\.cursor\hooks.json.llm-notch.bak.20260711T110300" `
  "$env:USERPROFILE\.cursor\hooks.json" -Force
```
