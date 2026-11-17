<#
.SYNOPSIS
    Nightly database backup for the App Directory server (PLAN.md 9.4).

.DESCRIPTION
    Runs pg_dump in custom format through the running Postgres container and writes
    appdir-<timestamp>.dump into infra\backups. Files older than 30 days are pruned.
    PostgreSQL's custom format is already compressed, so no gzip step is needed.

    Schedule on the VM host with Task Scheduler:
      schtasks /create /tn "App Directory backup" /sc daily /st 01:00 ^
        /tr "powershell -ExecutionPolicy Bypass -File <repo>\infra\scripts\backup-db.ps1"

.PARAMETER KeepDays
    Retention window in days. Defaults to 30.

.EXAMPLE
    .\infra\scripts\backup-db.ps1
    .\infra\scripts\backup-db.ps1 -KeepDays 14
#>

[CmdletBinding()]
param(
    [int]$KeepDays = 30
)

$ErrorActionPreference = "Stop"

$repo = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$envFile = Join-Path $repo ".env"
if (-not (Test-Path $envFile)) {
    throw "Missing $envFile - the backup needs the database credentials."
}

$values = @{}
Get-Content $envFile | ForEach-Object {
    if ($_ -match '^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$') {
        $values[$matches[1]] = $matches[2].Trim()
    }
}

$container = "app-directory-db-1"
$user = $values["POSTGRES_USER"]
$database = $values["POSTGRES_DB"]
$password = $values["POSTGRES_PASSWORD"]
if (-not $user -or -not $database -or -not $password) {
    throw "POSTGRES_USER, POSTGRES_PASSWORD or POSTGRES_DB is missing from .env."
}

$running = docker inspect -f "{{.State.Running}}" $container 2>$null
if ($running -ne "true") {
    throw "Container $container is not running. Start the stack first."
}

$backups = Join-Path $repo "infra\backups"
New-Item -ItemType Directory -Force -Path $backups | Out-Null

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$target = Join-Path $backups "appdir-$stamp.dump"

# Write the dump inside the container, then copy it out: piping stdout through PowerShell
# would decode the binary as text and corrupt it.
$remote = "/tmp/appdir-backup.dump"
docker exec -e "PGPASSWORD=$password" $container pg_dump -U $user -d $database --format=custom --file=$remote
if ($LASTEXITCODE -ne 0) {
    throw "pg_dump failed with exit code $LASTEXITCODE"
}

docker cp "${container}:$remote" $target | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "docker cp failed with exit code $LASTEXITCODE"
}
docker exec $container rm -f $remote | Out-Null

if (-not (Test-Path $target)) {
    throw "Backup file was not created at $target"
}

$size = (Get-Item $target).Length
$cutoff = (Get-Date).AddDays(-$KeepDays)
$pruned = @(Get-ChildItem $backups -Filter "appdir-*.dump" | Where-Object { $_.LastWriteTime -lt $cutoff })
foreach ($file in $pruned) {
    Remove-Item $file.FullName -Force
}

Write-Output ("backup: {0} ({1:N2} MB)" -f $target, ($size / 1MB))
if ($pruned.Count -gt 0) {
    Write-Output ("pruned: {0} file(s) older than {1} days" -f $pruned.Count, $KeepDays)
}