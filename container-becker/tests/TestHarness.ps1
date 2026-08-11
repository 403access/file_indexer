# =============================================================================
# TestHarness.ps1 - aggregator for the Crefo scenario-test support code.
# Dot-sourced by Run-CrefoTests.ps1 (executes in the caller's script scope, so
# $script: variables and $mockScript/$exportScript are shared).
#
# Initializes the shared test state, then dot-sources each per-concern part
# from the TestHarness/ subfolder (all into this same scope):
#
#   Fixtures      - TestData/ loading + Select-TestWealth / New-MockScenario
#   MockLifecycle - Get-FreePort / Start-Mock / Stop-Mock
#   ExporterRun   - New-RunConfig / Invoke-CrefoExportRun
#   Readers       - Read-CsvRows / Read-RequestLog / Get-CountValue
#   Assertions    - Write-Check / Assert-Equal / Assert-Contains /
#                   Assert-CsvRowsMatch
#   PhaseRunner   - Invoke-CrefoPhase (one exporter run against one mock
#                   snapshot)
#
# NOTE: in a dot-sourced file $PSScriptRoot points at that file's own
# directory, so the parts must not rely on it; $script:TestHarnessRoot is set
# here (the tests/ dir) and used by the parts instead.
# =============================================================================

# Shared state (resolves to the runner's script scope via dot-sourcing).
$script:TestHarnessRoot = $PSScriptRoot
$script:TestDataDir = Join-Path $script:TestHarnessRoot 'TestData'
$script:results = New-Object System.Collections.Generic.List[object]
$script:phaseFailures = New-Object System.Collections.Generic.List[string]

$harnessDir = Join-Path $script:TestHarnessRoot 'TestHarness'
. (Join-Path $harnessDir 'Fixtures.ps1')
. (Join-Path $harnessDir 'MockLifecycle.ps1')
. (Join-Path $harnessDir 'ExporterRun.ps1')
. (Join-Path $harnessDir 'Readers.ps1')
. (Join-Path $harnessDir 'Assertions.ps1')
. (Join-Path $harnessDir 'PhaseRunner.ps1')