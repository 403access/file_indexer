# =============================================================================
# TestHarness/ExporterRun.ps1 - exporter config writing + child-process runs.
# Dot-sourced via TestHarness.ps1 into the runner's scope.
#
# WHY CONFIG PER PHASE (New-RunConfig + Invoke-CrefoPhase)
# --------------------------------------------------------
# The exporter reads its BaseUrl from config at startup. Every phase starts a
# mock on a FRESH ephemeral port, so the port cannot be known until the mock is
# actually listening. New-RunConfig is therefore called by Invoke-CrefoPhase
# AFTER Start-Mock returns, writing a config.psd1 that points at that phase's
# mock. It is written into the runtime dir (not a temp file) so the dir is
# self-contained and easy to inspect/debug after a failed phase.
#
# WHY MANUAL PSD1 SERIALIZATION
# -----------------------------
# Options include Export-ModuleMember/ConvertTo-PSD1 or Invoke-Expression, but
# handwritten lines avoid: (a) Export-Clixml's binary-ish format, (b) any
# expression-evaluation risk that passable-but-garbage config values would hit.
# Booleans MUST be emitted as $true/$false (bare True/False are strings to the
# exporter's config parser), and numbers as bare ints; everything else is
# single-quoted so values with special characters pass through untouched.
# =============================================================================

# Writes a config.psd1 for the exporter pointing at the mock + a fresh runtime
# directory; returns the config path.
function New-RunConfig {
    param(
        [string]$RuntimeDir,
        [string]$BaseUrl,               # this phase's mock Base (see note above)
        [hashtable]$Overrides = @{}     # scenario-level config (e.g. SyncMode)
    )
    # Ensure the standard runtime sub-directories exist; the exporter may not
    # create output/state/logs/archive itself if they are missing.
    foreach ($d in @('output', 'state', 'logs', 'archive')) {
        $p = Join-Path $RuntimeDir $d
        if (-not (Test-Path -LiteralPath $p)) { New-Item -ItemType Directory -Path $p -Force | Out-Null }
    }
    # Sensible defaults for a test baseline: no delays, tolerant retries, small
    # page size (so pagination scenarios matter), and a known output filename
    # that the expectations can then read back.
    $defaults = [ordered]@{
        BaseUrl             = $BaseUrl
        Username            = 'tester'
        Password            = 'tester'
        ClientId            = 'testclient'
        ClientSecret        = 'testsecret'
        PageSize            = 50
        RequestDelayMs      = 0        # never slow the suite down on purpose
        MaxRetries          = 2        # retry tolerance under fault scenarios
        LogLevel            = 'INFO'
        RefreshAccountList  = $true
        FreeLineFromBalance = $false
        UseLastLimitDecisions = $true
        ArchiveRequests     = $false   # must stay off: archive dir is per-run
        SyncMode            = 'Incremental'
        MaxAgeDays          = 0
        RefetchRanges       = ''       # default: no forced re-fetch ranges
        OutputFileName      = 'crefo_limits.csv'
        OutputDir           = (Join-Path $RuntimeDir 'output')
        StateDir            = (Join-Path $RuntimeDir 'state')
        LogDir              = (Join-Path $RuntimeDir 'logs')
        ArchiveDir          = (Join-Path $RuntimeDir 'archive')
    }
    # Scenario overrides win over the defaults; any key the scenario sets lands
    # in the written config verbatim.
    foreach ($k in $Overrides.Keys) { $defaults[$k] = $Overrides[$k] }
    $path = Join-Path $RuntimeDir 'config.psd1'

    # Build the psd1 literal line-by-line (see "why manual serialization" above).
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('@{')
    foreach ($k in $defaults.Keys) {
        $v = $defaults[$k]
        $serialized = if ($v -is [bool]) { if ($v) { '$true' } else { '$false' } }
                      elseif ($v -is [int]) { [string]$v }   # numeric literals
                      else { "'{0}'" -f ([string]$v) }       # quote everything else
        $lines.Add(("    {0} = {1}" -f $k, $serialized))
    }
    $lines.Add('}')
    Set-Content -LiteralPath $path -Value ($lines -join "`n") -Encoding UTF8
    return $path
}

# Runs Start-CrefoExport.ps1 as a child process; returns the exit code and the
# path of the run's log file.
#
# The exporter is a SEPARATE pwsh process from the harness (Start-Process), not
# a function call, so:
#   - exit codes are the real ones ($LASTEXITCODE of the exporter script),
#     which the 'ExitCode' expectation asserts against;
#   - the exporter's non-interactive lifecycle (token refresh, snapshots, CSV
#     writing) runs exactly as in production.
# stdout/stderr are redirected to temp files (-RedirectStandardOutput must
# point at separate files; pwsh forbids sharing one between the two streams).
function Invoke-CrefoExportRun {
    param(
        [string]$ConfigPath,
        [string]$RuntimeDir,
        [string]$Name,          # run label; used only for logging/temp naming
        [hashtable]$Flags = @{} # -Reset / -ForceToken / -RefetchRanges mapping
    )
    # Fresh temp files per run avoid collisions when phases share a RuntimeDir
    # and make input-output pairing trivial to eyeball after a failure.
    $logFile = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-run-{0}-out.txt" -f ([guid]::NewGuid().ToString('N')))
    $errFile = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-run-{0}-err.txt" -f ([guid]::NewGuid().ToString('N')))
    $args = @('-NoProfile', '-File', $exportScript, '-ConfigPath', $ConfigPath)
    # Translate the simple scenario flags into the exporter's switch params;
    # RefetchRanges is a STRING argument (e.g. '1014,2021'), not a switch.
    if ($Flags['Reset']) { $args += '-Reset' }
    if ($Flags['ForceToken']) { $args += '-ForceToken' }
    if (-not [string]::IsNullOrWhiteSpace([string]$Flags['RefetchRanges'])) {
        $args += '-RefetchRanges'; $args += [string]$Flags['RefetchRanges']
    }
    $p = Start-Process -FilePath 'pwsh' -ArgumentList $args -Wait -PassThru `
        -RedirectStandardOutput $logFile -RedirectStandardError $errFile
    # The exporter writes a timestamped log under LogDir; THAT file is the
    # authoritative record (console mirror may buffer INFO lines differently),
    # so the phase's LogContains searches read the newest *.log in the dir.
    $runLog = @(Get-ChildItem -LiteralPath (Join-Path $RuntimeDir 'logs') -Filter '*.log' | Sort-Object LastWriteTime | Select-Object -Last 1)
    return [pscustomobject]@{
        ExitCode = $p.ExitCode
        OutFile = $logFile
        ErrFile = $errFile
        LogPath = if ($runLog.Count -gt 0) { $runLog[0].FullName } else { '' }
    }
}