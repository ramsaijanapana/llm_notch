# Example generated diff (Cursor project scope)

Illustrative output from `preview_connector_change` before applying the Cursor template. Line numbers and paths are examples.

```diff
--- a/Users/dev/projects/llm_notch/.cursor/hooks.json
+++ b/Users/dev/projects/llm_notch/.cursor/hooks.json
@@ -1,12 +1,44 @@
 {
   "version": 1,
   "hooks": {
     "afterFileEdit": [
       {
         "command": ".cursor/hooks/format.sh"
       }
+    ],
+    "sessionStart": [
+      {
+        "command": "sh sh integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event sessionStart",
+        "timeout": 2
+      }
+    ],
+    "preToolUse": [
+      {
+        "command": "sh integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event preToolUse",
+        "timeout": 2
+      }
+    ],
+    "postToolUse": [
+      {
+        "command": "sh integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event postToolUse",
+        "timeout": 2
+      }
+    ],
+    "postToolUseFailure": [
+      {
+        "command": "sh integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event postToolUseFailure",
+        "timeout": 2
+      }
+    ],
+    "stop": [
+      {
+        "command": "sh integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event stop",
+        "timeout": 2
+      }
+    ],
+    "sessionEnd": [
+      {
+        "command": "sh integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event sessionEnd",
+        "timeout": 2
+      }
     ]
   }
 }
```

## Preview metadata (dashboard)

```json
{
  "planId": "plan_8f3c2a1b",
  "source": "cursor",
  "scope": "project",
  "targetPath": "/Users/dev/projects/llm_notch/.cursor/hooks.json",
  "targetSha256": "a1b2c3...",
  "backupPath": "/Users/dev/projects/llm_notch/.cursor/hooks.json.llm-notch.bak.20260711T110300",
  "addedEvents": [
    "sessionStart",
    "preToolUse",
    "postToolUse",
    "postToolUseFailure",
    "stop",
    "sessionEnd"
  ],
  "preservedEvents": ["afterFileEdit"],
  "expiresAtMs": 1700000300000
}
```

## User confirmation copy (example)

> **Review integration change**  
> Scope: Cursor project hooks  
> Adds 6 observation-only hooks. Existing `afterFileEdit` hook is preserved.  
> llm_notch will **not** approve or block Cursor tools.  
> Backup: `.cursor/hooks.json.llm-notch.bak.20260711T110300`

## Abort example (hash mismatch)

```json
{
  "error": "FILE_CHANGED_SINCE_PREVIEW",
  "message": ".cursor/hooks.json changed after preview. Re-run preview.",
  "expectedSha256": "a1b2c3...",
  "actualSha256": "d4e5f6..."
}
```

No file is written.
