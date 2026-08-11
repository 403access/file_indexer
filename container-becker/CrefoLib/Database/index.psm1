# =============================================================================
# CrefoLib/Database/index.psm1 - entry point for the Database submodule.
# Dot-sources the implementation files and re-exports the public surface so
# callers can import a single module.
# =============================================================================

$script:SqliteExe = $null
$script:DbPath    = $null
$script:DbBuffer  = $null
$script:DbBatch   = 0

. (Join-Path $PSScriptRoot 'Sqlite.ps1')
. (Join-Path $PSScriptRoot 'Schema.ps1')
. (Join-Path $PSScriptRoot 'Writes.ps1')
. (Join-Path $PSScriptRoot 'Queries.ps1')

Export-ModuleMember -Function 'Invoke-CrefoSqlite', 'Initialize-CrefoDatabase', 'Complete-CrefoDatabase', 'Save-CrefoAccount', 'Save-CrefoRiskSnapshot', 'Save-CrefoApiExchange', 'Save-CrefoApiExchangeLog', 'Import-CrefoDatabaseFromState', 'Get-CrefoDatabaseCsvRows', 'Get-CrefoDatabaseStats', 'Get-CrefoDatabaseAccountSummary'
