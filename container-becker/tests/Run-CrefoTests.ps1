# =============================================================================
# Run-CrefoTests.ps1 - scenario tests for Start-CrefoExport.ps1.
#
# Every scenario drives the REAL exporter (a child pwsh process, so exit codes
# are meaningful) against the local Mock-CrefoApi.ps1 server. Mock responses
# are composed per scenario from the shared fixtures in TestData/ plus fault
# injection, so each feature can be triggered deterministically:
#
#   fresh sync / incremental reuse / delta discovery / probe fallback /
#   401 token refresh / transient retry / failed-account retry / refetch
#   ranges / RefreshAll / short-circuit / open-limit pipeline / bulk-call
#   fallback / free-line-from-balance / pagination / reset
#
# Usage:
#   pwsh -File tests/Run-CrefoTests.ps1            # run everything
#   pwsh -File tests/Run-CrefoTests.ps1 -Filter reuse   # only matching scenarios
#
# Exit code: 0 when all scenarios pass, 1 when at least one fails. Requires
# pwsh 7 (uses compress json logs + UTF-8 without BOM by default).
# =============================================================================

[CmdletBinding()]
param(
    [string]$Filter = ''          # substring filter on scenario names
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$mockScript = Join-Path $PSScriptRoot 'Mock-CrefoApi.ps1'
$exportScript = Join-Path $root 'Start-CrefoExport.ps1'
if (-not (Test-Path -LiteralPath $mockScript)) { throw "Mock not found: $mockScript" }
if (-not (Test-Path -LiteralPath $exportScript)) { throw "Exporter not found: $exportScript" }

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

function Get-FreePort {
    $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $l.Start()
    $port = ([System.Net.IPEndPoint]$l.LocalEndpoint).Port
    $l.Stop()
    return $port
}

# Loads one of the TestData/ JSON fixtures into a hashtable/array.
function Get-TestFixture {
    param([string]$Name)
    $path = Join-Path $PSScriptRoot ("TestData\{0}" -f $Name)
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

$script:results = New-Object System.Collections.Generic.List[object]
$script:phaseFailures = New-Object System.Collections.Generic.List[string]

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

function Read-CsvRows {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return @() }
    # ';'-separated, UTF-8 BOM, one header line. Import-Csv handles the header.
    return @(Import-Csv -LiteralPath $Path -Delimiter ';')
}

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

function Get-CountValue {
    param([string]$Path, [string]$Key)
    if (-not (Test-Path -LiteralPath $Path)) { return 0 }
    $c = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($null -eq $c.PSObject.Properties[$Key]) { return 0 }
    return [int]$c.$Key
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

# ---------------------------------------------------------------------------
# Assertion wrapper for one phase (one exporter run against one mock snapshot)
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

# ---------------------------------------------------------------------------
# Scenario definitions
# ---------------------------------------------------------------------------

$scenarios = @()

$scenarios += @{
    Name = 'fresh-sync'
    FilterTags = @('fresh')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014, 2044)
                CsvRowCount = 2
            }
        }
    )
}

$scenarios += @{
    Name = 'incremental-reuse'
    FilterTags = @('reuse', 'incremental')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044); CsvRowCount = 2 }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{
                ExitCode = 0
                RiskIds = @()               # unchanged -> zero /risk calls
                CsvRowCount = 2
                LogContains = @('Debtor list unchanged')
            }
        }
    )
}

$scenarios += @{
    Name = 'delta-new-account'
    FilterTags = @('delta')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044, 3050); DecisionIds = @(1014, 2044, 3050) }
            Expect = @{
                ExitCode = 0
                RiskIds = @(3050)            # only the new debtor gets /risk
                CsvRowCount = 3
                LogContains = @('Debtor list grew by 1', 'Delta from account list: 1 new account')
            }
        }
    )
}

$scenarios += @{
    Name = 'probe-failure-fallback'
    FilterTags = @('probe')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0; MaxRetries = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Probe500 = $true } }
            Expect = @{
                ExitCode = 0
                RiskIds = @()                # accounts already processed -> reused
                CsvRowCount = 2
                LogContains = @('falling back to full list sync')
            }
        }
    )
}

$scenarios += @{
    Name = 'v401-token-refresh'
    FilterTags = @('401', 'refresh')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Risk401OnceIds = @(1014) } }
            Expect = @{
                ExitCode = 0
                # 1014 was 401-once (2 attempts) + 2044 = 3 /risk calls total
                RiskIds = @(1014, 1014, 2044)
                CsvRowCount = 2
                LogContains = @('HTTP 401', 'refreshing access token')
            }
        }
    )
}

$scenarios += @{
    Name = 'transient-retry-500'
    FilterTags = @('retry')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0; MaxRetries = 2 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Risk500OnceId = 1014 } }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014, 1014, 2044)  # 1014 failed once then succeeded
                CsvRowCount = 2
                LogContains = @('Transient error HTTP 500')
            }
        }
    )
}

$scenarios += @{
    Name = 'failed-account-retry'
    FilterTags = @('failed', 'retry')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Risk500Ids = @(2044) } }
            Expect = @{
                ExitCode = 1          # 2044 permanently fails -> run fails
                CsvNotId = 2044       # failed accounts produce no CSV row
                LogContains = @('Debitor 2044 failed')
            }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{
                ExitCode = 0          # continues after the failed account recovers
                RiskIds = @(2044)     # failed account is retried
                CsvRowCount = 2
                LogContains = @('Debtor list unchanged')
            }
        }
    )
}

$scenarios += @{
    Name = 'refetch-ranges'
    FilterTags = @('refetch', 'range')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Flags = @{ RefetchRanges = '1014' }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014)     # only the forced range is re-fetched
                CsvRowCount = 2
                LogContains = @('Forcing /risk refetch for debtor id range(s): 1014')
            }
        }
    )
}

$scenarios += @{
    Name = 'refresh-all-mode'
    FilterTags = @('refreshall', 'refresh')
    Config = @{ SyncMode = 'RefreshAll'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }  # RefreshAll: always both
        }
    )
}

$scenarios += @{
    Name = 'decision-removed-refetch'
    FilterTags = @('decision', 'removed')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 4102); DecisionIds = @(1014) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 4102); CsvRowCount = 2 }
        },
        @{
            Mock = @{ AccountIds = @(1014, 4102); DecisionIds = @(1014) }
            Expect = @{
                ExitCode = 0
                # 4102 still holds an active snapshot (limit 104102 / code A)
                # but has no completed decision -> the "decision removed" rule
                # re-fetches it; 1014 is unchanged and reused.
                RiskIds = @(4102)
                CsvRowCount = 2
                CsvRows = @(@{ Id = 4102; Limit = '104102,00'; Code = 'A'; Gekauft = '5102,00'; Free = '99000,00' })
                LogContains = @('completed decision removed, account previously had a limit')
            }
        }
    )
}

$scenarios += @{
    Name = 'open-limit-pipeline'
    FilterTags = @('desire', 'open')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            # 5010 has no completed decision but an in-progress desire
            Mock = @{ AccountIds = @(1014, 5010); DecisionIds = @(1014); DesireIds = @(5010) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 5010); CsvRowCount = 2 }
        },
        @{
            Mock = @{ AccountIds = @(1014, 5010); DecisionIds = @(1014); DesireIds = @(5010) }
            Expect = @{
                ExitCode = 0
                RiskIds = @(5010)      # account in the open pipeline keeps refreshing
                CsvRowCount = 2
                LogContains = @('open limit pipeline')
            }
        }
    )
}

$scenarios += @{
    Name = 'bulk-call-fallback'
    FilterTags = @('bulk', 'decisions')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Decisions500 = $true; Desires500 = $true } }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014, 2044)   # bulk context unavailable -> full /risk pass
                CsvRowCount = 2
                LogContains = @('falling back to fetching /risk for every account')
            }
        }
    )
}

$scenarios += @{
    Name = 'free-line-from-balance'
    FilterTags = @('freeline', 'balance')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0; FreeLineFromBalance = $true }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014); DecisionIds = @(1014) }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014)
                CsvRowCount = 1
                # free line = limit - balance = 100000 - 80000 = 20000 (not -Gekauft)
                CsvRows = @(@{ Id = 1014; Free = '20000,00' })
            }
        }
    )
}

$scenarios += @{
    Name = 'pagination-many-accounts'
    FilterTags = @('page', 'pagination')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0; PageSize = 2 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044, 3050, 4102, 5010); DecisionIds = @(1014, 2044, 3050) }
            Expect = @{ ExitCode = 0; CsvRowCount = 5 }
        }
    )
}

$scenarios += @{
    Name = 'reset-reprocesses'
    FilterTags = @('reset')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044); CsvRowCount = 2 }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Flags = @{ Reset = $true }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014, 2044)     # reprocessed from scratch
                CsvRowCount = 2
                LogContains = @('Reset requested')
            }
        }
    )
}

# ---------------------------------------------------------------------------
# Run everything
# ---------------------------------------------------------------------------

$selected = @($scenarios | Where-Object {
    [string]::IsNullOrWhiteSpace($Filter) -or
    ($_.Name -like "*$Filter*") -or
    ($_.FilterTags -contains $Filter)
})

$index = 0
foreach ($scenario in $selected) {
    $index++
    Write-Host ""
    Write-Host ("===== [{0}/{1}] {2} =====" -f $index, $selected.Count, $scenario.Name)
    $runtime = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-tests-{0}-{1}" -f $scenario.Name, [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $runtime -Force | Out-Null

    $configOverrides = @{}
    foreach ($k in $scenario.Config.Keys) { $configOverrides[$k] = $scenario.Config[$k] }

    $phaseIndex = 0
    $scenarioPass = $true
    $allLines = New-Object System.Collections.Generic.List[string]
    foreach ($phase in $scenario.Phases) {
        $phaseIndex++
        $phaseResult = Invoke-CrefoPhase -Phase $phase -RuntimeDir $runtime -RunName ("phase{0}" -f $phaseIndex) -ConfigOverrides $configOverrides
        foreach ($line in $phaseResult.Lines) { $allLines.Add($line) }
        if (-not $phaseResult.Pass) { $scenarioPass = $false }
    }

    $allLines.Add(("  SCENARIO {0}: {1}" -f $scenario.Name, $(if ($scenarioPass) { 'PASS' } else { 'FAIL' })))
    foreach ($line in $allLines) { Write-Host $line }
    $script:results.Add([pscustomobject]@{ Name = $scenario.Name; Pass = $scenarioPass; Lines = @($allLines.ToArray()) })
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

Write-Host ""
Write-Host "=================================================================="
$passCount = @($script:results | Where-Object { $_.Pass }).Count
$failCount = @($script:results | Where-Object { -not $_.Pass }).Count
Write-Host ("TESTS: {0} passed, {1} failed, {2} total" -f $passCount, $failCount, $script:results.Count)
foreach ($r in $script:results) {
    Write-Host ("  {0,-28} {1}" -f $r.Name, $(if ($r.Pass) { 'PASS' } else { 'FAIL' }))
}
if ($failCount -gt 0) { exit 1 }
exit 0