<#
.SYNOPSIS
    Nightly backup of the App Directory CSV store.

.DESCRIPTION
    Archives infra\data (users.csv, apps.csv, settings.csv, icons, ...) into
    infra\backups\appdir-<timestamp>.zip. Files older than 30 days are pruned.
    Run from any directory; the repository root is resolved from this script.

    Schedule on the VM host with Task Scheduler:
      schtasks /create /tn "App Directory backup" /sc daily /st 01:00 ^
        /tr "powershell -ExecutionPolicy Bypass -File <repo>\infra\scripts\backup-db.ps1"

.PARAMETER KeepDays
    Retention window in days. Defaults to 30.
#>

[CmdletBinding()]
param([int]$KeepDays = 30)

$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$data = Join-Path $repo "infra\data"
$backups = Join-Path $repo "infra\backups"
if (-not (Test-Path $data)) {
    throw "No CSV store at $data. Start the API once to create it."
}

New-Item -ItemType Directory -Force -Path $backups | Out-Null

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$target = Join-Path $backups "appdir-$stamp.zip"
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory(
    $data,
    $target,
    [System.IO.Compression.CompressionLevel]::Optimal,
    $false
)

$cutoff = (Get-Date).AddDays(-$KeepDays)
$old = @(Get-ChildItem $backups -Filter "appdir-*.zip" | Where-Object { $_.LastWriteTime -lt $cutoff })
foreach ($file in $old) {
    Remove-Item $file.FullName -Force
}

Write-Output ("backup: {0} ({1:N2} MB)" -f $target, ((Get-Item $target).Length / 1MB))
if ($old.Count -gt 0) {
    Write-Output ("pruned: {0} archive(s) older than {1} days" -f $old.Count, $KeepDays)
}
