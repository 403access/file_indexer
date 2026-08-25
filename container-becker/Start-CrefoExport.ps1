# =============================================================================
# Start-CrefoExport.ps1
# Orchestrates the Crefo Factoring limit export (daily sync):
#   1. authenticate (OAuth2 password flow, cached token)
#   2. fetch the debitor account list (paginated) and merge into state; when
#      the database already holds entries, probe the size with pageSize=1 and
#      fetch only the delta instead of a full sync
#   3. fetch the limit-workflow bulk endpoints (last-limit-decisions +
#      open-limit-desires): accounts with no limit context short-circuit to a
#      0,00 / N row without a per-debtor /risk request
#   4. decide per account whether its stored risk snapshot is stale (new,
#      failed, decision changed, account in the open pipeline, or older than
#      MaxAgeDays) and only then call /risk
#   5. rebuild the full CSV from the account snapshots (stable order by id)
#   6. persist progress after every account so runs are resumable
#
# This file is the thin orchestration shell. The per-phase logic lives in the
# feature files under src/CrefoLib/Export/, dot-sourced below so they run in the
# script scope and can share state ($script:cfg, $script:token, ...):
#   src/CrefoLib/Export/Environment.ps1  config + logging + archive + DB setup
#   src/CrefoLib/Export/Token.ps1        OAuth2 token bootstrap + 401 refresher
#   src/CrefoLib/Export/Accounts.ps1     state load, reset, DB seeding, account
#                                        list discovery / delta sync
#   src/CrefoLib/Export/Run.ps1          bulk limit context, per-account loop,
#                                        CSV rebuild, run summary
#
# SyncMode:
#   Incremental (default) - few requests most days; /risk only where the
#     decision changed or the snapshot is older than MaxAgeDays.
#   RefreshAll           - like the original one-time sync: /risk for every
#     account with a limit context each run.
# Run with:  pwsh -File Start-CrefoExport.ps1 [-Reset] [-ForceToken] [-ConfigPath <path>]
# Exit code: 0 = success, 1 = at least one account failed (re-run to retry).
# =============================================================================

[CmdletBinding()]
param(
    [string]$ConfigPath = (Join-Path $PSScriptRoot 'config.psd1'),  # path to config.psd1
    [switch]$Reset,                                                # reprocess all accounts from scratch
    [switch]$ForceToken,                                           # ignore cached token, re-authenticate
    [string]$RefetchRanges = ''                                    # e.g. "1014,1100-1200"; forces /risk for those debtor ids/ranges
)

# Fail fast: any unhandled error stops the script instead of continuing blind.
$ErrorActionPreference = 'Stop'

# Modules are imported into the global scope so that functions inside one
# module (e.g. the API module calling the logger or the archive) can see each
# other.
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Logger.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Config.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\StateStore.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Snapshot.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\CsvFormat.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\Database\index.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\ApiArchive.psm1') -Global -Force
Import-Module -Name (Join-Path $PSScriptRoot 'src\CrefoLib\CrefoApi.psm1') -Global -Force

# Dot-source the exporter's per-phase orchestration. They are plain scripts
# (not a module) because the phases communicate through script-scope variables
# that must stay visible to each other and to the root script below.
. (Join-Path $PSScriptRoot 'src\CrefoLib\Export\Environment.ps1')
. (Join-Path $PSScriptRoot 'src\CrefoLib\Export\Token.ps1')
. (Join-Path $PSScriptRoot 'src\CrefoLib\Export\Accounts.ps1')
. (Join-Path $PSScriptRoot 'src\CrefoLib\Export\Run.ps1')

# Run the phases in order. Each phase sets up the state the next one needs
# ($script:cfg, $script:token/authRefresher, $script:state/statePath).
Initialize-CrefoExportEnvironment -ConfigPath $ConfigPath -RefetchRanges $RefetchRanges
Initialize-CrefoExportToken -ForceToken:$ForceToken
Sync-CrefoAccountList -Reset:$Reset
exit (Invoke-CrefoExportRun)