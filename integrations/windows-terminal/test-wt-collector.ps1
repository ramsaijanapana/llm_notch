# Smoke test for llm-notch-wt-collector.ps1 (no Pester dependency).
# Usage: pwsh -NoProfile -File integrations/windows-terminal/test-wt-collector.ps1

$ErrorActionPreference = 'Stop'

$collector = Join-Path $PSScriptRoot '..\wrappers\llm-notch-wt-collector.ps1'
if (-not (Test-Path -LiteralPath $collector)) {
    Write-Error "collector script not found: $collector"
    exit 1
}

function Assert-Equal([string] $Name, $Expected, $Actual) {
    if ($Expected -ne $Actual) {
        Write-Error "$Name expected '$Expected' but got '$Actual'"
        exit 1
    }
}

# Isolate process env for the test harness.
$preserve = @{
    WT_SESSION = $env:WT_SESSION
    LLM_NOTCH_TERMINAL_SESSION_ID = $env:LLM_NOTCH_TERMINAL_SESSION_ID
    LLM_NOTCH_TAB_ID = $env:LLM_NOTCH_TAB_ID
    LLM_NOTCH_PANE_ID = $env:LLM_NOTCH_PANE_ID
    LLM_NOTCH_WINDOW_HANDLE = $env:LLM_NOTCH_WINDOW_HANDLE
}
foreach ($key in $preserve.Keys) { Remove-Item "Env:$key" -ErrorAction SilentlyContinue }

try {
    . $collector

    $empty = Export-LlmNotchWtCollectorEnv
    Assert-Equal 'empty.terminalSessionId' $null $empty.terminalSessionId
    Assert-Equal 'empty.tabId' $null $empty.tabId

    $env:WT_SESSION = '5720ee6d-6474-47b0-88db-fa7e10e60d37'
    $withSession = Export-LlmNotchWtCollectorEnv
    Assert-Equal 'session' '5720ee6d-6474-47b0-88db-fa7e10e60d37' $withSession.terminalSessionId
    Assert-Equal 'env mirror' '5720ee6d-6474-47b0-88db-fa7e10e60d37' $env:LLM_NOTCH_TERMINAL_SESSION_ID

    Remove-Item Env:LLM_NOTCH_TAB_ID -ErrorAction SilentlyContinue
    Remove-Item Env:LLM_NOTCH_PANE_ID -ErrorAction SilentlyContinue
    $withLayout = Export-LlmNotchWtCollectorEnv -TabId '2' -PaneId '1'
    Assert-Equal 'tab override' '2' $withLayout.tabId
    Assert-Equal 'pane override' '1' $withLayout.paneId

    $env:LLM_NOTCH_TAB_ID = '9'
    $preserved = Export-LlmNotchWtCollectorEnv -TabId '2'
    Assert-Equal 'tab preserved' '9' $preserved.tabId

    Write-Host 'wt-collector smoke test passed'
}
finally {
    foreach ($entry in $preserve.GetEnumerator()) {
        if ($null -eq $entry.Value) {
            Remove-Item "Env:$($entry.Key)" -ErrorAction SilentlyContinue
        }
        else {
            Set-Item -Path "Env:$($entry.Key)" -Value $entry.Value
        }
    }
}
