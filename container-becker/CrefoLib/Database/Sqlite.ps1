# =============================================================================
# CrefoLib/Database/Sqlite.ps1 - low-level sqlite3 CLI wrapper.
# Dot-sourced by CrefoLib/Database/index.psm1.
# =============================================================================

function Resolve-CrefoSqlite {
    [CmdletBinding()]
    param([string]$SqlitePath = '')
    if ($script:SqliteExe) { return $script:SqliteExe }
    $exe = if ([string]::IsNullOrWhiteSpace($SqlitePath)) {
        (Get-Command 'sqlite3' -ErrorAction Stop).Source
    }
    else { $SqlitePath }
    $script:SqliteExe = $exe
    return $exe
}

function Invoke-CrefoSqlite {
    [CmdletBinding()]
    param(
        [string]$DbPath,
        [Parameter(Mandatory = $true)][string]$Sql,
        [string]$SqlitePath = '',
        [switch]$AsJson
    )
    $exe = Resolve-CrefoSqlite -SqlitePath $SqlitePath
    $args = New-Object System.Collections.Generic.List[string]
    $args.Add('-bail')
    if ($AsJson) { $args.Add('-json') }
    $args.Add($DbPath)
    $args.Add($Sql)
    $output = & $exe @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        $detail = ($output -join ' ').Trim()
        throw ("sqlite3 failed (exit {0}): {1}" -f $LASTEXITCODE, $detail)
    }
    if ($AsJson) {
        if (-not [string]::IsNullOrWhiteSpace([string]$output)) {
            return @(ConvertFrom-Json ([string]$output -join ''))
        }
        return @()
    }
    return $output
}
