# =============================================================================
# CrefoLib/Export/Environment.ps1 - run-environment bootstrap for the exporter.
# Dot-sourced by Start-CrefoExport.ps1 into the script scope so it can share
# $script:cfg, $script:logFile, $script:dbPath and friends with the other
# Export feature files. Only defines functions; nothing runs at dot-source time.
#
# Initialize-CrefoExportEnvironment:
#   1. import + merge config (defaults, env vars, dirs, validation)
#   2. apply the -RefetchRanges command-line override and parse the ranges
#   3. set up the per-run log file + logger and print the startup banner
#   4. initialize the API exchange archive and the local SQLite database
# =============================================================================

function Initialize-CrefoExportEnvironment {
    [CmdletBinding()]
    param(
        [string]$ConfigPath,
        [string]$RefetchRanges
    )

    # -------------------------------------------------------------------------
    # Configuration loading & merging (defaults, env vars, dirs, validation)
    # -------------------------------------------------------------------------
    $script:cfg = Import-CrefoConfig -ConfigPath $ConfigPath

    # Command-line override: -RefetchRanges wins over the config file.
    if (-not [string]::IsNullOrWhiteSpace($RefetchRanges)) {
        $script:cfg['RefetchRanges'] = $RefetchRanges
    }

    # Parse + validate the forced refetch ranges once; they are matched per account
    # in the processing loop (empty = no forced refetches this run).
    $script:forceRanges = @(ConvertTo-CrefoRefetchRanges -Value $script:cfg['RefetchRanges'])
    $script:forceRangesText = @($script:forceRanges | ForEach-Object {
        if ($_.Min -eq $_.Max) { '{0}' -f $_.Min } else { '{0}-{1}' -f $_.Min, $_.Max }
    }) -join ', '

    # -------------------------------------------------------------------------
    # Logging setup
    # -------------------------------------------------------------------------

    # One log file per run (timestamped) so a run's history is never overwritten.
    $script:logFile = Join-Path $script:cfg['LogDir'] ("crefo_export_{0}.log" -f (Get-Date -Format 'yyyyMMdd_HHmmss'))
    Initialize-Logger -LogFilePath $script:logFile -Level $script:cfg['LogLevel'] -Console $true

    Write-CrefoInfo ("=== Crefo Factoring limit export started ===")
    Write-CrefoInfo ("Config      : " + $ConfigPath)
    Write-CrefoInfo ("Base URL    : " + $script:cfg['BaseUrl'])
    Write-CrefoInfo ("Sync mode   : " + $script:cfg['SyncMode'] + "  (max age " + $script:cfg['MaxAgeDays'] + "d)")
    Write-CrefoInfo ("Output CSV  : " + (Join-Path $script:cfg['OutputDir'] $script:cfg['OutputFileName']))
    Write-CrefoInfo ("State file  : " + (Join-Path $script:cfg['StateDir'] 'crefo_state.json'))
    Write-CrefoInfo ("Log file    : " + $script:logFile)
    Write-CrefoInfo ("Archive dir : " + $script:cfg['ArchiveDir'] + "  (enabled=" + $script:cfg['ArchiveRequests'] + ")")
    if ($script:forceRanges.Count -gt 0) {
        Write-CrefoInfo ("Forcing /risk refetch for debtor id range(s): {0}" -f $script:forceRangesText)
    }

    # Persist every API request/response/data exchange to disk (configurable).
    Initialize-ApiArchive -Enabled $script:cfg['ArchiveRequests'] -RootDir $script:cfg['ArchiveDir']

    # Local SQLite store: canonical source of truth for the CSV. The JSON state is
    # still written alongside (rollback), but the CSV is rebuilt from the database.
    $script:dbPath = Join-Path $script:cfg['StateDir'] 'crefo.db'
    Initialize-CrefoDatabase -DbPath $script:dbPath
    Write-CrefoInfo ("Database    : " + $script:dbPath)
}