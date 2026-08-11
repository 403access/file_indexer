# =============================================================================
# TestHarness/PhaseRunner.ps1 - one exporter run against one mock snapshot.
# Dot-sourced via TestHarness.ps1 into the runner's scope.
# =============================================================================

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