# llm-notch-hook-wrapper.ps1
# Portable PowerShell hook wrapper. Always fails open in hook mode.
#
# Usage:
#   pwsh -NoProfile -File llm-notch-hook-wrapper.ps1 -Source cursor -VendorEvent sessionStart
#   (vendor JSON on stdin)
#
# Environment:
#   LLM_NOTCH_HOOK_BIN            — helper path (default: llm-notch-hook)
#   LLM_NOTCH_HOOK_TIMEOUT_SEC    — max wait seconds (default: 2)

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('cursor', 'claudeCode', 'codex', 'generic')]
    [string] $Source,

    [Parameter(Mandatory = $true)]
    [string] $VendorEvent,

    [int] $TimeoutSec = $(if ($env:LLM_NOTCH_HOOK_TIMEOUT_SEC) { [int]$env:LLM_NOTCH_HOOK_TIMEOUT_SEC } else { 2 })
)

function Write-FailOpen {
    Write-Output '{}'
    exit 0
}

$helper = if ($env:LLM_NOTCH_HOOK_BIN) { $env:LLM_NOTCH_HOOK_BIN } else { 'llm-notch-hook' }

if (-not (Get-Command $helper -ErrorAction SilentlyContinue)) {
    Write-FailOpen
}

$stdin = [Console]::In.ReadToEnd()
if ($null -eq $stdin) { $stdin = '' }

$tmp = [System.IO.Path]::GetTempFileName()
try {
    [System.IO.File]::WriteAllText($tmp, $stdin)

    $argList = @(
        'hook',
        '--source', $Source,
        '--vendor-event', $VendorEvent,
        '--hook-mode'
    )

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $helper
    $psi.Arguments = ($argList -join ' ')
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true

    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi

    if (-not $proc.Start()) {
        Write-FailOpen
    }

    $payload = [System.IO.File]::ReadAllText($tmp)
    $proc.StandardInput.Write($payload)
    $proc.StandardInput.Close()

    if (-not $proc.WaitForExit($TimeoutSec * 1000)) {
        try { $proc.Kill($true) } catch { }
    }
}
catch {
    # Swallow — hook mode must not block the agent.
}
finally {
    if (Test-Path $tmp) { Remove-Item -Force $tmp }
}

Write-FailOpen
