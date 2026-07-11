#Requires -Version 5.1
<#
.SYNOPSIS
  Authenticode-sign llm_notch Windows release artifacts.

.DESCRIPTION
  Scaffold for release pipelines. Requires env vars:
    WINDOWS_CERTIFICATE_BASE64  — base64-encoded PFX
    WINDOWS_CERTIFICATE_PASSWORD
    SIGNING_TIMESTAMP_URL       — optional; defaults to DigiCert

  Never commit certificates or passwords.

.EXAMPLE
  $env:WINDOWS_CERTIFICATE_BASE64 = Get-Content .\cert.pfx.b64 -Raw
  $env:WINDOWS_CERTIFICATE_PASSWORD = '***'
  .\scripts\signing\sign-windows.ps1 -ArtifactPath .\src-tauri\target\release\bundle\nsis\llm_notch_0.1.0_x64-setup.exe
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactPath,

    [string]$TimestampUrl = 'http://timestamp.digicert.com'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $ArtifactPath)) {
    throw "Artifact not found: $ArtifactPath"
}

$certB64 = $env:WINDOWS_CERTIFICATE_BASE64
$certPassword = $env:WINDOWS_CERTIFICATE_PASSWORD
if ([string]::IsNullOrWhiteSpace($certB64) -or [string]::IsNullOrWhiteSpace($certPassword)) {
    Write-Error @'
Signing secrets missing. Set WINDOWS_CERTIFICATE_BASE64 and WINDOWS_CERTIFICATE_PASSWORD.
CI smoke builds skip signing; this script is a release gate scaffold only.
'@
    exit 2
}

$pfxBytes = [Convert]::FromBase64String($certB64)
$securePassword = ConvertTo-SecureString -String $certPassword -AsPlainText -Force
$cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(
    $pfxBytes,
    $securePassword,
    [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::EphemeralKeySet
)

try {
    Set-AuthenticodeSignature -FilePath $ArtifactPath -Certificate $cert -TimestampServer $TimestampUrl | Out-Null
    $signature = Get-AuthenticodeSignature -FilePath $ArtifactPath
    if ($signature.Status -ne 'Valid') {
        throw "Authenticode status: $($signature.Status)"
    }
    Write-Host "Signed: $ArtifactPath ($($signature.Status))"
}
finally {
    $cert.Dispose()
}
