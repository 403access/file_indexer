# =============================================================================
# TestHarness.ps1 - reusable helpers + the phase runner for the Crefo scenario
# tests. Dot-sourced by Run-CrefoTests.ps1 (executes in the caller's script
# scope, so $script: variables and $mockScript/$exportScript are shared).
#
# Provides:
#   - fixture loading (TestData/) + per-phase mock scenario composition
#   - Mock-CrefoApi.ps1 process lifecycle (Start-Mock / Stop-Mock)
#   - exporter child-process runs (Invoke-CrefoExportRun)
#   - assertion primitives (Write-Check / Assert-*)
#   - Invoke-CrefoPhase: one exporter run against one mock snapshot
# =============================================================================

# Per-phase/global assertion state (shared with the runner's scope via
# dot-sourcing; $script: also resolves to the runner's script scope).
$script:TestDataDir = Join-Path $PSScriptRoot 'TestData'
$script:results = New-Object System.Collections.Generic.List[object]
$script:phaseFailures = New-Object System.Collections.Generic.List[string]

# Loads one of the TestData/ JSON fixtures into a hashtable/array.
function Get-TestFixture {
    param([string]$Name)
    $path = Join-Path $script:TestDataDir $Name
    return @(Get-Content -LiteralPath $path -Raw | ConvertFrom-Json)
}

$TestAccounts = Get-TestFixture 'accounts.json'
$TestDecisions = Get-TestFixture 'decisions.json'
$TestDesires = Get-TestFixture 'desires.json'
$TestRisks = @{}
foreach ($entry in (Get-TestFixture 'risks.json').PSObject.Properties) {
    $TestRisks[[string]$entry.Name] = $entry.Value
}

# Picks the accounts/decisions/desires subsets for a mock from the fixtures.
function Select-TestWealth {
    param(
        [int[]]$AccountIds,
        [int[]]$DecisionIds = @(),
        [int[]]$DesireIds = @()
    )
    return [pscustomobject]@{
        Accounts  = @($TestAccounts | Where-Object { $_.id -in $AccountIds })
        Decisions = @($TestDecisions | Where-Object { $_.debtorNumber -in $DecisionIds })
        Desires   = @($TestDesires | Where-Object { $_.debtorNumber -in $DesireIds })
        Risks     = $TestRisks
    }
}

# Writes a mock scenario json for Mock-CrefoApi.ps1 from a phase definition.
function New-MockScenario {
    param(
        [object]$Wealth,                 # Select-TestWealth result
        [string]$RiskFile = '',          # optional per-id risk overrides json path
        [hashtable]$Faults = @{}
    )
    $scenario = @{
        Accounts  = @($Wealth.Accounts)
        Decisions = @($Wealth.Decisions)
        Desires   = @($Wealth.Desires)
        Risk      = @{}
        Faults    = $Faults
    }
    foreach ($id in @($Wealth.Accounts).id) {
        $risk = $Wealth.Risks[[string]$id]
        if ($null -ne $risk) { $scenario.Risk[[string]$id] = $risk }
    }
    $path = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-mock-{0}.json" -f ([guid]::NewGuid().ToString('N')))
    Set-Content -LiteralPath $path -Value ($scenario | ConvertTo-Json -Depth 20) -Encoding UTF8
    return $path
}

# Gets a free TCP port for a mock server.
function Get-FreePort {
    $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $l.Start()
    $port = ([System.Net.IPEndPoint]$l.LocalEndpoint).Port
    $l.Stop()
    return $port
}

# Starts Mock-CrefoApi.ps1 on a free port and waits until it is ready.
function Start-Mock {
    param(
        [string]$MockFile,
        [string]$RequestLog,
        [string]$CountFile
    )
    $actualPort = Get-FreePort
    $ready = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-ready-{0}.txt" -f ([guid]::NewGuid().ToString('N')))
    $stop = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-stop-{0}.txt" -f ([guid]::NewGuid().ToString('N')))
    Remove-Item -LiteralPath $stop -Force -ErrorAction SilentlyContinue

    $proc = Start-Process -FilePath 'pwsh' -ArgumentList @(
        '-NoProfile', '-File', $mockScript,
        '-Port', $actualPort,
        '-MockFile', $mockFile,
        '-RequestLog', $RequestLog,
        '-CountFile', $CountFile,
        '-ReadyFile', $ready,
        '-StopFile', $stop) -PassThru

    $ok = $false
    for ($i = 0; $i -lt 200; $i++) {
        if ($proc.HasExited) { break }
        if (Test-Path -LiteralPath $ready) { $ok = $true; break }
        Start-Sleep -Milliseconds 50
    }
    if (-not $ok) {
        throw "Mock server did not become ready (port $actualPort)."
    }
    return [pscustomobject]@{
        Port = $actualPort
        ReadyFile = $ready
        StopFile = $stop
        Process = $proc
        Base = "http://127.0.0.1:$actualPort"
    }
}

function Stop-Mock {
    param([object]$Mock)
    New-Item -ItemType File -Path $Mock.StopFile -Force | Out-Null
    for ($i = 0; $i -lt 100; $i++) {
        if ($Mock.Process.HasExited) { break }
        Start-Sleep -Milliseconds 50
    }
    if (-not $Mock.Process.HasExited) { Stop-Process -Id $Mock.Process.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $Mock.ReadyFile, $Mock.StopFile -Force -ErrorAction SilentlyContinue
}

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

# Reads the ;-separated CSV into row objects.
function Read-CsvRows {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return @() }
    return @(Import-Csv -LiteralPath $Path -Delimiter ';')
}

# Reads the mock's JSON-lines request log.
function Read-RequestLog {
    param([string]$Path)
    $rows = @()
    if (Test-Path -LiteralPath $Path) {
        foreach ($line in Get-Content -LiteralPath $Path) {
            if ([string]::IsNullOrWhiteSpace($line)) { continue }
            try { $rows += ($line | ConvertFrom-Json) } catch { }
        }
    }
    return $rows
}

# Reads one counter value from the mock's count file.
function Get-CountValue {
    param([string]$Path, [string]$Key)
    if (-not (Test-Path -LiteralPath $Path)) { return 0 }
    $c = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($null -eq $c.PSObject.Properties[$Key]) { return 0 }
    return [int]$c.$Key
}

# ---------------------------------------------------------------------------
# Assertion primitives. Each check appends an "[OK ]"/"[FAIL]" line to
# $script:phaseFailures; a failing line fails the current phase.
# ---------------------------------------------------------------------------

function Write-Check {
    param([string]$What, [bool]$Ok, [string]$Detail)
    $script:phaseFailures.Add($(if ($Ok) { "  [OK ] {0}" -f $What } else { "  [FAIL] {0}: {1}" -f $What, $Detail })) | Out-Null
}

function Assert-Equal {
    param([string]$What, [object]$Expected, [object]$Actual)
    $e = [array]$Expected
    $a = [array]$Actual
    Write-Check $What ($e.Count -eq $a.Count -and @(Compare-Object $e $a -SyncWindow 0).Count -eq 0) ("expected [{0}] got [{1}]" -f ($e -join ','), ($a -join ','))
}

function Assert-Contains {
    param([string]$What, [object[]]$Needle, [object[]]$Haystack, [string]$Mode = 'all')
    $found = @($Needle | Where-Object { $Haystack -contains $_ })
    $ok = if ($Mode -eq 'all') { $found.Count -eq $Needle.Count } else { $found.Count -gt 0 }
    Write-Check $What $ok ("need {0} -> found {1}" -f ($Needle -join ','), ($found -join ','))
}

function Assert-CsvRowsMatch {
    param([object[]]$Actual, [object[]]$Expected)
    foreach ($want in $Expected) {
        $line = $Actual | Where-Object { $_.'Kto-Nr.' -eq ([string]$want.Id) }
        if ($null -eq $line) {
            Write-Check ("CSV row {0} present" -f $want.Id) $false 'row missing'
            continue
        }
        if ($want.ContainsKey('Limit'))    { $a = $line.Limit.TrimEnd("`r").TrimEnd("`n").Trim(); Write-Check ("CSV row {0} Limit" -f $want.Id) ($a -eq [string]$want.Limit) ("expected {0} got {1}" -f $want.Limit, $a) }
        if ($want.ContainsKey('Code'))     { $a = $line.LimitKennz.Trim(); Write-Check ("CSV row {0} Code" -f $want.Id) ($a -eq [string]$want.Code) ("expected {0} got {1}" -f $want.Code, $a) }
        if ($want.ContainsKey('Gekauft'))  { $a = $line.Gekauft.Trim(); Write-Check ("CSV row {0} Gekauft" -f $want.Id) ($a -eq [string]$want.Gekauft) ("expected {0} got {1}" -f $want.Gekauft, $a) }
        if ($want.ContainsKey('Free'))     { $a = $line.'freie Linie'.Trim(); Write-Check ("CSV row {0} freie Linie" -f $want.Id) ($a -eq [string]$want.Free) ("expected {0} got {1}" -f $want.Free, $a) }
    }
}

# ---------------------------------------------------------------------------
# Phase runner: one exporter run against one mock snapshot
# ---------------------------------------------------------------------------

function Invoke-CrefoPhase {
    param(
        [hashtable]$Phase,        # Mock, Flags, Expect
        [string]$RuntimeDir,
        [string]$RunName,
        [hashtable]$ConfigOverrides
    )
    $phaseFailures.Clear()
    $wealth = Select-TestWealth -AccountIds $Phase.Mock.AccountIds `
        -DecisionIds $(if ($Phase.Mock.ContainsKey('DecisionIds')) { $Phase.Mock.DecisionIds } else { @() }) `
        -DesireIds $(if ($Phase.Mock.ContainsKey('DesireIds')) { $Phase.Mock.DesireIds } else { @() })
    $mockFile = New-MockScenario -Wealth $wealth -RiskFile '' -Faults $(if ($Phase.Mock.ContainsKey('Faults')) { $Phase.Mock.Faults } else { @{} })
    $reqLog = Join-Path $RuntimeDir ("requests-{0}.jsonl" -f $RunName)
    $cntFile = Join-Path $RuntimeDir ("counts-{0}.json" -f $RunName)
    Remove-Item -LiteralPath $reqLog, $cntFile -Force -ErrorAction SilentlyContinue

    $mock = Start-Mock -MockFile $mockFile -RequestLog $reqLog -CountFile $cntFile
    try {
        # The exporter config must point at THIS phase's mock base URL, so it is
        # written here, after the mock has been started on its free port.
        $cfgPath = New-RunConfig -RuntimeDir $RuntimeDir -BaseUrl $mock.Base -Overrides $ConfigOverrides
        $run = Invoke-CrefoExportRun -ConfigPath $cfgPath -RuntimeDir $RuntimeDir -Name $RunName -Flags $(if ($Phase.ContainsKey('Flags')) { $Phase.Flags } else { @{} })
    }
    finally {
        Stop-Mock -Mock $mock
    }
    Remove-Item -LiteralPath $mockFile -Force -ErrorAction SilentlyContinue

    # --- expectations --------------------------------------------------------
    $expect = $Phase.Expect
    $logText = ''
    if ($run.LogPath -and (Test-Path -LiteralPath $run.LogPath)) { $logText = Get-Content -LiteralPath $run.LogPath -Raw }
    $reqRows = Read-RequestLog -Path $reqLog
    $riskIds = @($reqRows | Where-Object { $_.path -match '/risk$' } | Select-Object -ExpandProperty debitor)
    $csv = Read-CsvRows -Path (Join-Path $RuntimeDir 'output\crefo_limits.csv')

    foreach ($key in $expect.Keys) {
        switch ($key) {
            'ExitCode'    { Assert-Equal 'ExitCode' $expect[$key] $run.ExitCode }
            'RiskIds'     { Assert-Equal 'risk fetches (debitor ids)' $expect[$key] $riskIds }
            'CsvRowCount' { Assert-Equal 'CSV data rows' $expect[$key] @($csv).Count }
            'CsvRows'     { Assert-CsvRowsMatch -Actual $csv -Expected $expect[$key] }
            'CsvNotId'    { $present = @($csv | Where-Object { $_.'Kto-Nr.' -eq ([string]$expect[$key]) }); Write-Check ("CSV does not contain {0}" -f $expect[$key]) ($present.Count -eq 0) ('row present') }
            'LogContains' { foreach ($needle in @($expect[$key])) { Write-Check ("log contains '{0}'" -f $needle) ($logText.Contains($needle)) ('missing from log') } }
        }
    }

    # All failed check lines of the phase -> phase failed.
    $failedLines = @($phaseFailures | Where-Object { $_.Contains('[FAIL]') })
    $phaseResult = [pscustomobject]@{
        Name = $RunName
        Pass = ($failedLines.Count -eq 0)
        Lines = @($phaseFailures.ToArray())
    }
    return $phaseResult
}