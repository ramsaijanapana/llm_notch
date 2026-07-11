# Validate JSON syntax for integrations/fixtures and template JSON files.
# Uses the first available validator; adds no repo dependencies.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$fixtures = Join-Path $root 'fixtures'

function Get-JsonValidator {
    if (Get-Command python3 -ErrorAction SilentlyContinue) { return 'python3' }
    if (Get-Command node -ErrorAction SilentlyContinue) { return 'node' }
    if (Get-Command ruby -ErrorAction SilentlyContinue) { return 'ruby' }
    if (Get-Command jq -ErrorAction SilentlyContinue) { return 'jq' }
    return $null
}

function Test-JsonFile {
    param([string] $Path, [string] $Validator)
    switch ($Validator) {
        'python3' { & python3 -m json.tool $Path | Out-Null }
        'node' { & node -e "JSON.parse(require('fs').readFileSync(process.argv[1],'utf8'))" $Path | Out-Null }
        'ruby' { & ruby -rjson -e "JSON.parse(File.read(ARGV[0]))" $Path | Out-Null }
        'jq' { & jq -e . $Path | Out-Null }
    }
}

$validator = Get-JsonValidator
if (-not $validator) {
    Write-Error 'No JSON validator found (tried python3, node, ruby, jq).'
}

$files = @(
    Get-ChildItem -Path $fixtures -Filter '*.json' -Recurse -File | ForEach-Object { $_.FullName }
    (Join-Path $root 'cursor/hooks.json.template')
    (Join-Path $root 'claude-code/settings.hooks.template.json')
    (Join-Path $root 'codex/hooks.json.template')
) | Where-Object { Test-Path $_ }

$failed = 0
foreach ($file in $files) {
    try {
        Test-JsonFile -Path $file -Validator $validator
    }
    catch {
        Write-Host "INVALID: $file"
        $failed++
    }
}

Write-Host "Validated $($files.Count) JSON files using $validator."
if ($failed -gt 0) { exit 1 }
