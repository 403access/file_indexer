# =============================================================================
# Run-LimitlinieTests.ps1 - tests for the standalone Update-Limitlinie.ps1
# downloader/archiver. Every scenario drives the REAL script (a child pwsh
# process, so exit codes are meaningful) against the local Mock-CrefoApi.ps1
# server, using document-folder scenarios composed from simple psd1 tables.
#
# What is covered (and nothing more):
#   - Step A (archive): every file from the configured/API-derived folders is
#     downloaded into <ArchiveDir>/<yyyy>/<MM Monthname>/ and skipped when it
#     is already present at its target path.
#   - Folder selection: config 'DocumentFolders' wins; otherwise the API
#     list-directory response is used.
#   - Step B (limit line, UNCHANGED): the newest *_Limitlinie.csv becomes
#     kundenlimits.csv, older files in the download dir are removed, and a
#     missing candidate fails the run with a non-zero exit code.
#
# Layout (all dot-sourced into this script's scope):
#   TestHarness.ps1     - mock lifecycle / readers / assertions
#   LimitlinieHarness.ps1 - limitlinie-specific mock scenario + config + run
#
# Usage:
#   pwsh -File tests/Run-LimitlinieTests.ps1            # run everything
#   pwsh -File tests/Run-LimitlinieTests.ps1 -Filter skip   # only matching
#
# Exit code: 0 when all scenarios pass, 1 when at least one fails.
# =============================================================================

[CmdletBinding()]
param(
    [string]$Filter = ''          # substring filter on scenario names
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$mockScript = Join-Path $PSScriptRoot 'Mock-CrefoApi.ps1'
$limitlinieScript = Join-Path $root 'Update-Limitlinie.ps1'
if (-not (Test-Path -LiteralPath $mockScript)) { throw "Mock not found: $mockScript" }
if (-not (Test-Path -LiteralPath $limitlinieScript)) { throw "Update-Limitlinie.ps1 not found: $limitlinieScript" }

. (Join-Path $PSScriptRoot 'TestHarness.ps1')
. (Join-Path $PSScriptRoot 'LimitlinieHarness.ps1')

# ---------------------------------------------------------------------------
# Phase runner: one Update-Limitlinie run against one mock snapshot.
#   Phase.Mock  = @{ Folders = @(...); Files = @{ folder = @(@{name; created}) } }
#   Phase.Config = extra config keys (e.g. DocumentFolders, Directory)
#   Phase.Expect = @{
#       ExitCode        - expected process exit code
#       ArchiveFiles    - relative paths that must exist under ArchiveDir
#       ArchiveAbsent   - relative paths that must NOT exist under ArchiveDir
#       DownloadFiles   - names that must exist in DownloadDir
#       DownloadOnly    - name that must be the ONLY file in DownloadDir
#       KundenlimitsCreated - expected year folder of kundenlimits.csv? (n/a)
#       ReqLogContains  - exact strings that must appear in the request log
#       OutputContains  - exact strings that must appear in the script output
#   }
# ---------------------------------------------------------------------------
function Invoke-LimitliniePhase {
    param(
        [hashtable]$Phase,
        [string]$RuntimeDir,
        [string]$RunName
    )
    $phaseFailures.Clear()

    $mockFile = New-LimitlinieMockScenario -Folders $Phase.Mock.Folders -Files $Phase.Mock.Files
    $reqLog = Join-Path $RuntimeDir ("requests-{0}.jsonl" -f $RunName)
    $cntFile = Join-Path $RuntimeDir ("counts-{0}.json" -f $RunName)
    Remove-Item -LiteralPath $reqLog, $cntFile -Force -ErrorAction SilentlyContinue

    $mock = Start-Mock -MockFile $mockFile -RequestLog $reqLog -CountFile $cntFile
    try {
        $configOverrides = if ($Phase.ContainsKey('Config')) { $Phase.Config } else { @{} }
        $cfgPath = New-LimitlinieConfig -RuntimeDir $RuntimeDir -BaseUrl $mock.Base -Overrides $configOverrides
        $run = Invoke-LimitlinieRun -ConfigPath $cfgPath -ScriptPath $limitlinieScript
    }
    finally {
        Stop-Mock -Mock $mock
    }
    Remove-Item -LiteralPath $mockFile -Force -ErrorAction SilentlyContinue

    $archiveDir = Join-Path $RuntimeDir 'archive'
    $downloadDir = Join-Path $RuntimeDir 'download'
    $expect = $Phase.Expect

    foreach ($key in $expect.Keys) {
        switch ($key) {
            'ExitCode' { Assert-Equal 'exit code' $expect[$key] $run.ExitCode }
            'ArchiveFiles' {
                foreach ($rel in @($expect[$key])) {
                    $p = Join-Path $archiveDir $rel
                    Write-Check ("archive file {0}" -f $rel) (Test-Path -LiteralPath $p) 'missing'
                }
            }
            'ArchiveAbsent' {
                foreach ($rel in @($expect[$key])) {
                    $p = Join-Path $archiveDir $rel
                    Write-Check ("archive file absent {0}" -f $rel) (-not (Test-Path -LiteralPath $p)) "present at $p"
                }
            }
            'DownloadFiles' {
                foreach ($name in @($expect[$key])) {
                    $p = Join-Path $downloadDir $name
                    Write-Check ("download file {0}" -f $name) (Test-Path -LiteralPath $p) "missing at $p"
                }
            }
            'DownloadOnly' {
                $expected = [string]$expect[$key]
                $present = @(Get-ChildItem -LiteralPath $downloadDir -File -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name)
                Write-Check ("download dir contains only {0}" -f $expected) ($present.Count -eq 1 -and $present -contains $expected) ("found {0}" -f ($present -join ','))
            }
            'ReqLogContains' {
                $rows = Read-RequestLog -Path $reqLog
                foreach ($needle in @($expect[$key])) {
                    $hit = @($rows | Where-Object { ("{0} {1}" -f $_.method, $_.path).Contains($needle) })
                    Write-Check ("request log {0}" -f $needle) ($hit.Count -gt 0) 'not logged'
                }
            }
            'OutputContains' {
                foreach ($needle in @($expect[$key])) {
                    Write-Check ("output contains '{0}'" -f $needle) ($run.Output.Contains($needle)) 'missing from output'
                }
            }
        }
    }

    $failedLines = @($phaseFailures | Where-Object { $_.Contains('[FAIL]') })
    return [pscustomobject]@{
        Name = $RunName
        Pass = ($failedLines.Count -eq 0)
        Lines = @($phaseFailures.ToArray())
        ExitCode = $run.ExitCode
    }
}

# ---------------------------------------------------------------------------
# Scenario catalogue
# ---------------------------------------------------------------------------

function Add-LimitlinieScenario {
    param($Name, $Phases)
    $global:limitlinieScenarios += @{
        Name = $Name
        Phases = @($Phases)
    }
}
$global:limitlinieScenarios = @()

# --- 1) Archive downloads every file into year/month folders ----------------
# The config has no 'DocumentFolders', so the script must call list-directory,
# then archive files from EACH folder. Files are bunched by their created
# timestamp into German year/month folders; the newest limit-line CSV also
# ends up as kundenlimits.csv.
Add-LimitlinieScenario -Name 'limitlinie-archive-all-folders' -Phases @(
    @{
        Mock = @{
            Folders = @('Tagesabrechnungen', 'Monatsabrechnungen', 'Sonstiges', 'Mahnungen', 'Einreichungen')
            Files = @{
                'Tagesabrechnungen' = @(
                    @{ name = '20260811_1014_Limitlinie.csv'; created = '2026-08-11T21:25:08' },
                    @{ name = '20260811_1014_FinJournal.csv'; created = '2026-08-11T21:22:40' },
                    @{ name = '20260811_1014_TA.pdf'; created = '2026-08-11T21:20:00' }
                )
                'Monatsabrechnungen' = @(
                    @{ name = '202607_1014_KTO.pdf'; created = '2026-07-31T22:00:00' }
                )
                'Sonstiges' = @(
                    @{ name = '20260706_1014_OP.pdf'; created = '2026-07-06T10:12:00' }
                )
                'Mahnungen' = @(
                    @{ name = '20260707_1014_MahnPos.csv'; created = '2026-07-07T09:05:00' }
                )
                'Einreichungen' = @(
                    @{ name = '61-31.07.2026-19274_05.pdf'; created = '2026-07-31T14:30:00' }
                )
            }
        }
        Expect = @{
            ExitCode = 0
            ArchiveFiles = @(
                '2026/08 August/20260811_1014_Limitlinie.csv',
                '2026/08 August/20260811_1014_FinJournal.csv',
                '2026/08 August/20260811_1014_TA.pdf',
                '2026/07 Juli/202607_1014_KTO.pdf',
                '2026/07 Juli/20260706_1014_OP.pdf',
                '2026/07 Juli/20260707_1014_MahnPos.csv',
                '2026/07 Juli/61-31.07.2026-19274_05.pdf'
            )
            DownloadFiles = @('kundenlimits.csv')
            ReqLogContains = @(
                '/api/v1/Documents/list-directory',
                '/api/v1/Documents/Tagesabrechnungen/list-document',
                '/api/v1/Documents/Monatsabrechnungen/list-document',
                '/api/v1/Documents/Sonstiges/list-document',
                '/api/v1/Documents/Mahnungen/list-document',
                '/api/v1/Documents/Einreichungen/list-document',
                '/api/v1/Documents/Tagesabrechnungen/20260811_1014_Limitlinie.csv'
            )
            OutputContains = @('Archive folders: Tagesabrechnungen, Monatsabrechnungen, Sonstiges, Mahnungen, Einreichungen')
        }
    }
)

# --- 2) Config 'DocumentFolders' wins over the API list ---------------------
# With an explicit list the script must NOT call list-directory for the archive
# step; only the configured folder is archived.
Add-LimitlinieScenario -Name 'limitlinie-archive-config-folders' -Phases @(
    @{
        Mock = @{
            Folders = @('Tagesabrechnungen', 'Monatsabrechnungen')
            Files = @{
                'Tagesabrechnungen' = @(
                    @{ name = '20260811_1014_Limitlinie.csv'; created = '2026-08-11T21:25:08' }
                )
                'Monatsabrechnungen' = @(
                    @{ name = '202607_1014_KTO.pdf'; created = '2026-07-31T22:00:00' },
                    @{ name = '202607_1014_SAB.pdf'; created = '2026-07-31T22:01:00' }
                )
            }
        }
        Config = @{ DocumentFolders = @('Tagesabrechnungen') }
        Expect = @{
            ExitCode = 0
            ArchiveFiles = @('2026/08 August/20260811_1014_Limitlinie.csv')
            ArchiveAbsent = @('2026/07 Juli/202607_1014_KTO.pdf', '2026/07 Juli/202607_1014_SAB.pdf')
            DownloadFiles = @('kundenlimits.csv')
            OutputContains = @('Archive folders: Tagesabrechnungen')
        }
    }
)

# --- 3) Second run skips already-archived files -----------------------------
# Phase 1 archives everything into the year/month tree; phase 2 runs against
# the SAME archive dir and must download nothing new (all target paths exist),
# while the limit-line step still refreshes kundenlimits.csv.
Add-LimitlinieScenario -Name 'limitlinie-archive-skips-existing' -Phases @(
    @{
        Mock = @{
            Folders = @('Tagesabrechnungen')
            Files = @{
                'Tagesabrechnungen' = @(
                    @{ name = '20260811_1014_Limitlinie.csv'; created = '2026-08-11T21:25:08' }
                )
            }
        }
        Expect = @{
            ExitCode = 0
            ArchiveFiles = @('2026/08 August/20260811_1014_Limitlinie.csv')
            OutputContains = @('skipped=0')
        }
    },
    @{
        Mock = @{
            Folders = @('Tagesabrechnungen')
            Files = @{
                'Tagesabrechnungen' = @(
                    @{ name = '20260811_1014_Limitlinie.csv'; created = '2026-08-11T21:25:08' }
                )
            }
        }
        Expect = @{
            ExitCode = 0
            ArchiveFiles = @('2026/08 August/20260811_1014_Limitlinie.csv')
            OutputContains = @('skipped=1')
            DownloadFiles = @('kundenlimits.csv')
        }
    }
)

# --- 4) Limit-line step unchanged: newest wins, older downloads removed -----
# Two limit-line CSVs plus a foreign file in the download folder. kundenlimits
# must be the NEWEST candidate and the foreign file must be deleted.
Add-LimitlinieScenario -Name 'limitlinie-newest-and-cleanup' -Phases @(
    @{
        Mock = @{
            Folders = @('Tagesabrechnungen')
            Files = @{
                'Tagesabrechnungen' = @(
                    @{ name = '20260810_1014_Limitlinie.csv'; created = '2026-08-10T21:23:24' },
                    @{ name = '20260811_1014_Limitlinie.csv'; created = '2026-08-11T21:25:08' }
                )
            }
        }
        Expect = @{
            ExitCode = 0
            ArchiveFiles = @(
                '2026/08 August/20260810_1014_Limitlinie.csv',
                '2026/08 August/20260811_1014_Limitlinie.csv'
            )
            DownloadFiles = @('kundenlimits.csv')
            DownloadOnly = 'kundenlimits.csv'
            ReqLogContains = @(
                '/api/v1/Documents/Tagesabrechnungen/20260811_1014_Limitlinie.csv'
            )
        }
    }
)

# --- 5) No limit-line candidate fails the run -------------------------------
Add-LimitlinieScenario -Name 'limitlinie-missing-candidate-fails' -Phases @(
    @{
        Mock = @{
            Folders = @('Tagesabrechnungen')
            Files = @{
                'Tagesabrechnungen' = @(
                    @{ name = '20260811_1014_TA.pdf'; created = '2026-08-11T09:00:00' }
                )
            }
        }
        Expect = @{
            ExitCode = 1
            ArchiveFiles = @('2026/08 August/20260811_1014_TA.pdf')
            DownloadFiles = @()
            OutputContains = @('No file ending in')
        }
    }
)

# ---------------------------------------------------------------------------
# Run everything
# ---------------------------------------------------------------------------

$selected = @($limitlinieScenarios | Where-Object {
    [string]::IsNullOrWhiteSpace($Filter) -or ($_.Name -like "*$Filter*")
})

$script:results = New-Object System.Collections.Generic.List[object]
$index = 0
foreach ($scenario in $selected) {
    $index++
    Write-Host ""
    Write-Host ("===== [{0}/{1}] {2} =====" -f $index, $selected.Count, $scenario.Name)
    $runtime = Join-Path ([System.IO.Path]::GetTempPath()) ("limitlinie-tests-{0}-{1}" -f $scenario.Name, [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $runtime -Force | Out-Null

    $phaseIndex = 0
    $scenarioPass = $true
    $allLines = New-Object System.Collections.Generic.List[string]
    foreach ($phase in $scenario.Phases) {
        $phaseIndex++
        $phaseResult = Invoke-LimitliniePhase -Phase $phase -RuntimeDir $runtime -RunName ("phase{0}" -f $phaseIndex)
        foreach ($line in $phaseResult.Lines) { $allLines.Add($line) }
        if (-not $phaseResult.Pass) { $scenarioPass = $false }
    }

    $allLines.Add(("  SCENARIO {0}: {1}" -f $scenario.Name, $(if ($scenarioPass) { 'PASS' } else { 'FAIL' })))
    foreach ($line in $allLines) { Write-Host $line }
    $script:results.Add([pscustomobject]@{ Name = $scenario.Name; Pass = $scenarioPass; Lines = @($allLines.ToArray()) })
}

Write-Host ""
Write-Host "=================================================================="
$passCount = @($script:results | Where-Object { $_.Pass }).Count
$failCount = @($script:results | Where-Object { -not $_.Pass }).Count
Write-Host ("TESTS: {0} passed, {1} failed, {2} total" -f $passCount, $failCount, $script:results.Count)
foreach ($r in $script:results) {
    Write-Host ("  {0,-34} {1}" -f $r.Name, $(if ($r.Pass) { 'PASS' } else { 'FAIL' }))
}
if ($failCount -gt 0) { exit 1 }
exit 0