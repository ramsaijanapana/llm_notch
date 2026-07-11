# Safe generic emit examples — require llm-notch-hook on PATH and a running llm_notch host.
# Documentation samples only; not invoked by any installer.
$ErrorActionPreference = 'Stop'

$helper = if ($env:LLM_NOTCH_HOOK_BIN) { $env:LLM_NOTCH_HOOK_BIN } else { 'llm-notch-hook' }

if (-not (Get-Command $helper -ErrorAction SilentlyContinue)) {
    Write-Error 'llm-notch-hook not found; examples are inert.'
}

# Start a session and register this PowerShell process as its root.
$processStartedAtMs = [DateTimeOffset]::new((Get-Process -Id $PID).StartTime).ToUnixTimeMilliseconds()
& $helper emit --source generic --event sessionStart `
    --external-session-id generic-cli-7 --label 'Generic CLI agent' `
    --workspace-label llm_notch --status running --pid $PID `
    --process-started-at-ms $processStartedAtMs

# Append a redacted tool event.
& $helper emit --source generic --event tool `
    --external-session-id generic-cli-7 --summary 'Build step finished' --tool-name cargo

# Set observation-only attention.
& $helper emit --source generic --event attention `
    --external-session-id generic-cli-7 --attention question --summary 'Agent waiting for input'
