# =============================================================================
# CrefoLib/Inspect/Sqlite.ps1 - low-level sqlite3 CLI wrapper for inspection.
# Dot-sourced by CrefoLib/Inspect/index.psm1.
# =============================================================================

function Invoke-InspectSqlite {
    [CmdletBinding()]
    param(
        [string]$DbPath,
        [string]$Sql,
        [switch]$AsJson
    )
    if (-not (Test-Path -LiteralPath $DbPath)) {
        throw "Database not found: $DbPath"
    }
    $exe = Get-Command 'sqlite3' -ErrorAction Stop
    $args = @('-bail')
    if ($AsJson) { $args += '-json' }
    $args += $DbPath
    $args += $Sql
    $output = & $exe.Source @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        $detail = ($output -join ' ').Trim()
        throw "sqlite3 failed: $detail"
    }
    if ($AsJson -and $output) {
        return ConvertFrom-Json ($output -join '')
    }
    return $output
}
