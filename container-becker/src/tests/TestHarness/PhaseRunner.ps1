# =============================================================================
# TestHarness/PhaseRunner.ps1 - one exporter run against one mock snapshot.
# Dot-sourced via TestHarness.ps1 into the runner's scope.
#
# PHASE LIFECYCLE (the contract Invoke-CrefoPhase honors):
#   1. Clear the phase-failure scratchpad (a fresh phase gets a clean slate).
#   2. Compose the mock's "world" from the phase's Mock spec via
#      Select-TestWealth + New-MockScenario (a temp JSON snapshot file).
#   3. Start Mock-CrefoApi.ps1 on a free port (file handshake for readiness).
#   4. Write THIS phase's config.psd1 pointing at that mock's port.
#   5. Run the real exporter as a child process against it.
#   6. Stop the mock in a finally block (guaranteed even if the exporter threw
#      or the config was bad - a leaked mock process would wedge later phases).
#   7. Evaluate every expectation against the mock's request log, the produced
#      CSV and the exporter's LogDir log, accumulating verdict lines.
#   8. Snapshot the phase outcome as a small object the runner folds into the
#      scenario report.
#
# The RuntimeDir is SHARED across all phases of a scenario (the runner creates
# it once per scenario), so state/state.json and the sqlite db persist between
# phases: that is what makes incremental/delta/multi-phase scenarios possible.
# =============================================================================

# Runs one phase. $Phase is a hashtable with the documented keys:
#   Mock   = @{ AccountIds; DecisionIds; DesireIds; Faults }   - the mock world
#   Flags  = @{ Reset; ForceToken; RefetchRanges }             - run switches
#   Expect = @{ ExitCode; RiskIds; CsvRowCount; CsvRows; CsvNotId; LogContains }
# ConfigOverrides carry the SCENARIO-level config into this phase's config.
function Invoke-CrefoPhase {
    param(
        [hashtable]$Phase,        # Mock, Flags, Expect
        [string]$RuntimeDir,      # this scenario's shared runtime directory
        [string]$RunName,         # e.g. "phase1" - labels log/count/request files
        [hashtable]$ConfigOverrides
    )
    # 1. Fresh verdict stack for this phase (Write-Check appends here).
    $phaseFailures.Clear()
    # 2. Compose the mock snapshot; DecisionIds/DesireIds/Faults are optional
    #    in the phase definition, hence the ContainsKey guards below.
    $wealth = Select-TestWealth -AccountIds $Phase.Mock.AccountIds `
        -DecisionIds $(if ($Phase.Mock.ContainsKey('DecisionIds')) { $Phase.Mock.DecisionIds } else { @() }) `
        -DesireIds $(if ($Phase.Mock.ContainsKey('DesireIds')) { $Phase.Mock.DesireIds } else { @() })
    $mockFile = New-MockScenario -Wealth $wealth -RiskFile '' -Faults $(if ($Phase.Mock.ContainsKey('Faults')) { $Phase.Mock.Faults } else { @{} })
    # Per-phase observation files, named by RunName so multi-phase scenarios
    # keep phase1/phase2... artifacts distinct in the runtime dir.
    $reqLog = Join-Path $RuntimeDir ("requests-{0}.jsonl" -f $RunName)
    $cntFile = Join-Path $RuntimeDir ("counts-{0}.json" -f $RunName)
    Remove-Item -LiteralPath $reqLog, $cntFile -Force -ErrorAction SilentlyContinue

    # 3-5. Start mock, point config at it, run the exporter. The config MUST be
    #      written only after the mock's free port is known (see ExporterRun).
    $mock = Start-Mock -MockFile $mockFile -RequestLog $reqLog -CountFile $cntFile
    try {
        $cfgPath = New-RunConfig -RuntimeDir $RuntimeDir -BaseUrl $mock.Base -Overrides $ConfigOverrides
        $run = Invoke-CrefoExportRun -ConfigPath $cfgPath -RuntimeDir $RuntimeDir -Name $RunName -Flags $(if ($Phase.ContainsKey('Flags')) { $Phase.Flags } else { @{} })
    }
    finally {
        # 6. Always stop the mock, even when a step above threw.
        Stop-Mock -Mock $mock
    }
    # The mock snapshot temp file outlived its purpose; the runtime dir keeps
    # the pieces that matter (logs, config, state, CSV, request log).
    Remove-Item -LiteralPath $mockFile -Force -ErrorAction SilentlyContinue

    # 7. Gather the observable artifacts the expectations read.
    $expect = $Phase.Expect
    $logText = ''
    # LogContains searches the exporter's authoritative LogDir log; empty when
    # the exporter produced none (a scenario where that happens must assert on
    # exit code / artifacts instead - log search there would be vacuously false).
    if ($run.LogPath -and (Test-Path -LiteralPath $run.LogPath)) { $logText = Get-Content -LiteralPath $run.LogPath -Raw }
    $reqRows = Read-RequestLog -Path $reqLog
    # RiskIds derives from the /risk requests in the mock's log: the order of
    # appearance IS the fetch order, which Order-aware Assert-Equal preserves.
    $riskIds = @($reqRows | Where-Object { $_.path -match '/risk$' } | Select-Object -ExpandProperty debitor)
    # Backslashes keep it working identically on Windows and Unix; Join-Path
    # would produce the OS-native separator for / on Unix, and importing the
    # CSV is independent of how the path string reads.
    $csv = Read-CsvRows -Path (Join-Path $RuntimeDir 'output\crefo_limits.csv')

    # Evaluate each expectation key present in the phase (extra keys are legal
    # but unimplemented ones are silently skipped - add support below to use).
    # Each expected row becomes a set of Write-Check verdicts; CsvNotId inverts
    # presence (used to assert an account was NOT exported). LogContains flattens
    # its needle list: one check per expected log line.
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

    # 8. Any '[FAIL]' verdict line in the clean snapshot fails the phase.
    $failedLines = @($phaseFailures | Where-Object { $_.Contains('[FAIL]') })
    $phaseResult = [pscustomobject]@{
        Name = $RunName
        Pass = ($failedLines.Count -eq 0)
        Lines = @($phaseFailures.ToArray())   # snapshot - later phases must not leak lines backward
    }
    return $phaseResult
}