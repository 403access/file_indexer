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
#   ranges / RefreshAll / decision-removed refetch / open-limit pipeline /
#   bulk-call fallback / free-line-from-balance / pagination / reset
#
# Layout (all dot-sourced into this script's scope):
#   TestHarness.ps1   - aggregator for the support code; dot-sources the
#                       per-concern parts in TestHarness/ (mock lifecycle,
#                       exporter runs, readers, assertions, Invoke-CrefoPhase)
#   TestScenarios.ps1 - aggregator for the scenario table; dot-sources the
#                       feature-category files in scenarios/
#   Mock-CrefoApi.ps1 - the mock API server started per phase
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

. (Join-Path $PSScriptRoot 'TestHarness.ps1')
. (Join-Path $PSScriptRoot 'TestScenarios.ps1')

# ---------------------------------------------------------------------------
# Run everything
# ---------------------------------------------------------------------------

# Only the scenarios whose name matches -Filter (substring) OR whose FilterTags
# contain the exact token run; an empty filter selects the whole catalogue.
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
    # One RUNTIME dir per scenario (not per phase): state.json + the sqlite db
    # survive under it, so later phases of the scenario build on earlier ones.
    # The GUID suffix keeps parallel/debug runs from clobbering each other.
    $runtime = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-tests-{0}-{1}" -f $scenario.Name, [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $runtime -Force | Out-Null

    # Scenario-level config overrides apply to EVERY phase; per-phase Mock/
    # Flags/Expect still override/observe the run itself.
    $configOverrides = @{}
    foreach ($k in $scenario.Config.Keys) { $configOverrides[$k] = $scenario.Config[$k] }

    # Accumulate every phase's verdict lines into one ordered block, so the
    # printed scenario report is contiguous. Phase names phase1/phase2/...
    # keep multi-phase scenarios readable in that block.
    $phaseIndex = 0
    $scenarioPass = $true
    $allLines = New-Object System.Collections.Generic.List[string]
    foreach ($phase in $scenario.Phases) {
        $phaseIndex++
        $phaseResult = Invoke-CrefoPhase -Phase $phase -RuntimeDir $runtime -RunName ("phase{0}" -f $phaseIndex) -ConfigOverrides $configOverrides
        foreach ($line in $phaseResult.Lines) { $allLines.Add($line) }
        # A scenario passes only if EVERY phase passed (a failed phase's Lines
        # are already in $allLines; the final verdict line reflects the AND).
        if (-not $phaseResult.Pass) { $scenarioPass = $false }
    }

    $allLines.Add(("  SCENARIO {0}: {1}" -f $scenario.Name, $(if ($scenarioPass) { 'PASS' } else { 'FAIL' })))
    foreach ($line in $allLines) { Write-Host $line }
    # Snapshot into the shared summary accumulator (see TestHarness.ps1).
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