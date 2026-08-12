# =============================================================================
# CrefoLib/Inspect/index.psm1 - entry point for the Inspect submodule.
# Dot-sources the implementation files and re-exports the public surface so
# callers can import a single module.
# =============================================================================

$script:InspectDbPath = $null

. (Join-Path $PSScriptRoot 'Sqlite.ps1')
. (Join-Path $PSScriptRoot 'Queries.ps1')
. (Join-Path $PSScriptRoot 'Format.ps1')

Export-ModuleMember -Function 'Invoke-InspectSqlite', 'Get-DatabaseStats', 'Get-InspectAccount', 'Get-InspectAccountHistory', 'Show-DatabaseStats', 'Show-InspectAccountDetail', 'Show-InspectAccountList'
