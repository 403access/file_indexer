# =============================================================================
# TestHarness/ExporterRun.ps1 - exporter config writing + child-process runs.
# Dot-sourced via TestHarness.ps1 into the runner's scope.
# =============================================================================

# Writes a config.psd1 for the exporter pointing at the mock + a fresh runtime
# directory; returns the config path.
function New-RunConfig {
    param(
        [string]$RuntimeDir,
        [string]$BaseUrl,
        [hashtable]$Overrides = @{}
    )
    foreach ($d in @('output', 'state', 'logs', 'archive')) {
        $p = Join-Path $RuntimeDir $d
        if (-not (Test-Path -LiteralPath $p)) { New-Item -ItemType Directory -Path $p -Force | Out-Null }
    }
    $defaults = [ordered]@{
        BaseUrl             = $BaseUrl
        Username            = 'tester'
        Password            = 'tester'
        ClientId            = 'testclient'
        ClientSecret        = 'testsecret'
        PageSize            = 50
        RequestDelayMs      = 0
        MaxRetries          = 2
        LogLevel            = 'INFO'
        RefreshAccountList  = $true
        FreeLineFromBalance = $false
        UseLastLimitDecisions = $true
        ArchiveRequests     = $false
        SyncMode            = 'Incremental'
        MaxAgeDays          = 0
        RefetchRanges       = ''
        OutputFileName      = 'crefo_limits.csv'
        OutputDir           = (Join-Path $RuntimeDir 'output')
        StateDir            = (Join-Path $RuntimeDir 'state')
        LogDir              = (Join-Path $RuntimeDir 'logs')
        ArchiveDir          = (Join-Path $RuntimeDir 'archive')
    }
    foreach ($k in $Overrides.Keys) { $defaults[$k] = $Overrides[$k] }
    $path = Join-Path $RuntimeDir 'config.psd1'
    Set-Content -LiteralPath $path -Value ("@{0}`n" -f '')
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('@{')
    foreach ($k in $defaults.Keys) {
        $v = $defaults[$k]
        $serialized = if ($v -is [bool]) { if ($v) { '$true' } else { '$false' } }
                      elseif ($v -is [int]) { [string]$v }
                      else { "'{0}'" -f ([string]$v) }
        $lines.Add(("    {0} = {1}" -f $k, $serialized))
    }
    $lines.Add('}')
    Set-Content -LiteralPath $path -Value ($lines -join "`n") -Encoding UTF8
    return $path
}

# Runs Start-CrefoExport.ps1 as a child process; returns the exit code and the
# path of the run's log file.
function Invoke-CrefoExportRun {
    param(
        [string]$ConfigPath,
        [string]$RuntimeDir,
        [string]$Name,
        [hashtable]$Flags = @{}
    )
    $logFile = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-run-{0}-out.txt" -f ([guid]::NewGuid().ToString('N')))
    $errFile = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-run-{0}-err.txt" -f ([guid]::NewGuid().ToString('N')))
    $args = @('-NoProfile', '-File', $exportScript, '-ConfigPath', $ConfigPath)
    if ($Flags['Reset']) { $args += '-Reset' }
    if ($Flags['ForceToken']) { $args += '-ForceToken' }
    if (-not [string]::IsNullOrWhiteSpace([string]$Flags['RefetchRanges'])) {
        $args += '-RefetchRanges'; $args += [string]$Flags['RefetchRanges']
    }
    $p = Start-Process -FilePath 'pwsh' -ArgumentList $args -Wait -PassThru `
        -RedirectStandardOutput $logFile -RedirectStandardError $errFile
    # The exporter also writes a timestamped log under LogDir; that is the
    # authoritative content (console mirror may omit INFO in some setups).
    $runLog = @(Get-ChildItem -LiteralPath (Join-Path $RuntimeDir 'logs') -Filter '*.log' | Sort-Object LastWriteTime | Select-Object -Last 1)
    return [pscustomobject]@{
        ExitCode = $p.ExitCode
        OutFile = $logFile
        ErrFile = $errFile
        LogPath = if ($runLog.Count -gt 0) { $runLog[0].FullName } else { '' }
    }
}