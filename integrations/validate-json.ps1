# Validate JSON syntax for integrations/fixtures and template JSON files.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$fixtures = Join-Path $root 'fixtures'

function Test-JsonFile {
    param([string] $Path)
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json | Out-Null
}

$files = @(
    Get-ChildItem -Path $fixtures -Filter '*.json' -Recurse -File | ForEach-Object { $_.FullName }
    (Join-Path $root 'cursor/hooks.json.template')
    (Join-Path $root 'claude-code/settings.hooks.template.json')
    (Join-Path $root 'codex/hooks.json.template')
    (Join-Path $root 'gemini/settings.hooks.template.json')
    (Join-Path $root 'qwen/settings.hooks.template.json')
    (Join-Path $root 'antigravity-cli/hooks.json.template')
    (Join-Path $root 'copilot/hooks.json.template')
    (Join-Path $root 'remote/hooks.cursor.template.json')
) | Where-Object { Test-Path $_ }

$failed = 0
foreach ($file in $files) {
    try {
        Test-JsonFile -Path $file
    }
    catch {
        Write-Host "INVALID: $file"
        $failed++
    }
}

Write-Host "Validated $($files.Count) JSON files using ConvertFrom-Json."
if ($failed -gt 0) { exit 1 }
