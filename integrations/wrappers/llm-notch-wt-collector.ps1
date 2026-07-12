# llm-notch-wt-collector.ps1
# Honest Windows Terminal metadata collector for hook environments.
#
# Windows Terminal shell integration sets WT_SESSION (GUID per tab/pane connection).
# Tab and pane numeric indices are NOT published by Windows Terminal env vars today.
# This script only exports values that are already present or explicitly configured.
#
# Usage (PowerShell profile or Windows Terminal profile command line):
#   . "$PSScriptRoot\llm-notch-wt-collector.ps1"
#   Export-LlmNotchWtCollectorEnv
#
# Optional explicit indices (user-configured layout — not auto-discovered):
#   Export-LlmNotchWtCollectorEnv -TabId '1' -PaneId '0'
#
# See integrations/windows-terminal/README.md for limitations and setup.

function Export-LlmNotchWtCollectorEnv {
    [CmdletBinding()]
    param(
        [string] $TerminalSessionId,
        [string] $TabId,
        [string] $PaneId,
        [string] $WindowHandle
    )

    function Get-TrimmedEnv([string] $Name) {
        $raw = [Environment]::GetEnvironmentVariable($Name, 'Process')
        if ($null -eq $raw) { return $null }
        $trimmed = $raw.Trim()
        if ($trimmed.Length -eq 0) { return $null }
        return $trimmed
    }

    function Set-ProcessEnvIfAbsent([string] $Name, [string] $Value) {
        if ($null -eq $Value -or $Value.Trim().Length -eq 0) { return }
        if ($null -ne (Get-TrimmedEnv $Name)) { return }
        Set-Item -Path "Env:$Name" -Value $Value.Trim()
    }

    function Invoke-LlmNotchRustCollectorEnv {
        $helper = if ($env:LLM_NOTCH_HOOK_BIN) { $env:LLM_NOTCH_HOOK_BIN } else { 'llm-notch-hook' }
        if (-not (Get-Command $helper -ErrorAction SilentlyContinue)) { return }

        try {
            $json = & $helper collect-terminal-env 2>$null
            if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($json)) { return }
            $parsed = $json | ConvertFrom-Json
            if ($null -ne $parsed.terminalSessionId) {
                Set-ProcessEnvIfAbsent 'LLM_NOTCH_TERMINAL_SESSION_ID' "$($parsed.terminalSessionId)"
            }
            if ($null -ne $parsed.tabId) {
                Set-ProcessEnvIfAbsent 'LLM_NOTCH_TAB_ID' "$($parsed.tabId)"
            }
            if ($null -ne $parsed.paneId) {
                Set-ProcessEnvIfAbsent 'LLM_NOTCH_PANE_ID' "$($parsed.paneId)"
            }
            if ($null -ne $parsed.windowHandle) {
                Set-ProcessEnvIfAbsent 'LLM_NOTCH_WINDOW_HANDLE' "$($parsed.windowHandle)"
            }
        }
        catch {
            # Best-effort only; PowerShell collector remains honest when the helper is absent.
        }
    }

    Invoke-LlmNotchRustCollectorEnv

    $wtSession = Get-TrimmedEnv 'WT_SESSION'
    $explicitSession = Get-TrimmedEnv 'LLM_NOTCH_TERMINAL_SESSION_ID'

    if ($null -eq $explicitSession) {
        if ($null -ne $TerminalSessionId -and $TerminalSessionId.Trim().Length -gt 0) {
            Set-ProcessEnvIfAbsent 'LLM_NOTCH_TERMINAL_SESSION_ID' $TerminalSessionId
        }
        elseif ($null -ne $wtSession) {
            # Mirror WT_SESSION so downstream collectors have a stable override name.
            Set-ProcessEnvIfAbsent 'LLM_NOTCH_TERMINAL_SESSION_ID' $wtSession
        }
    }

    if ($null -ne $TabId -and $TabId.Trim().Length -gt 0) {
        Set-ProcessEnvIfAbsent 'LLM_NOTCH_TAB_ID' $TabId
    }
    if ($null -ne $PaneId -and $PaneId.Trim().Length -gt 0) {
        Set-ProcessEnvIfAbsent 'LLM_NOTCH_PANE_ID' $PaneId
    }
    if ($null -ne $WindowHandle -and $WindowHandle.Trim().Length -gt 0) {
        Set-ProcessEnvIfAbsent 'LLM_NOTCH_WINDOW_HANDLE' $WindowHandle
    }

    return [ordered]@{
        terminalSessionId = Get-TrimmedEnv 'LLM_NOTCH_TERMINAL_SESSION_ID'
        tabId             = Get-TrimmedEnv 'LLM_NOTCH_TAB_ID'
        paneId            = Get-TrimmedEnv 'LLM_NOTCH_PANE_ID'
        windowHandle      = Get-TrimmedEnv 'LLM_NOTCH_WINDOW_HANDLE'
        wtProfileId       = Get-TrimmedEnv 'WT_PROFILE_ID'
        wtProfileName     = Get-TrimmedEnv 'WT_PROFILE_NAME'
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    Export-LlmNotchWtCollectorEnv @PSBoundParameters | ConvertTo-Json -Compress
}
