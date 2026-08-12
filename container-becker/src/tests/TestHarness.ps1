# =============================================================================
# TestHarness.ps1 - aggregator for the Crefo scenario-test support code.
# Dot-sourced by Run-CrefoTests.ps1 (executes in the caller's script scope, so
# $script: variables and $mockScript/$exportScript are shared).
#
# SCOPE MODEL
# -----------
# Dot-sourcing runs the file in the CALLER's scope. That means everything the
# runner dot-sources (this file, the parts below, and TestScenarios.ps1)
# shares ONE script scope with Run-CrefoTests.ps1. Two consequences:
#
#   1. Functions and variables defined anywhere in the chain are callable /
#      readable everywhere in the chain. Run-CrefoTests.ps1 can call
#      Invoke-CrefoPhase and read $results/$scenarios directly.
#   2. The $script: prefix in this file resolves to that single shared scope.
#      We therefore centralize ALL shared mutable state here ($results,
#      $phaseFailures, $TestDataDir, $TestHarnessRoot) so the parts can rely
#      on it instead of trying to keep their own copies.
#
# All parts must be dot-sourced (via '.' operator) - NOT Import-Module -
# because a module would get its own $script: scope and its own copy of the
# state, breaking the shared-state contract above.
#
# NOTE: in a dot-sourced file $PSScriptRoot points at that file's OWN
# directory. The parts live in TestHarness/, so their $PSScriptRoot is the
# TestHarness/ folder, not tests/. They must therefore not resolve paths via
# $PSScriptRoot; instead they use $script:TestHarnessRoot, captured here
# (which IS the tests/ directory).
#
# FILE SPLIT (why several small files instead of one ~340-line script)
# --------------------------------------------------------------------
# The support code grows together with the scenario catalogue, so keeping
# every concern in one file made diffs noisy and navigation painful. Each part
# below has a single responsibility and a documented contract:
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
# ORDERING: state must be initialized before the parts are dot-sourced, and
# the parts may reference each other's functions freely at runtime (PowerShell
# resolves functions at call time, so dot-source order between parts is only
# relevant for readability - listed dependencies-first below).
# =============================================================================

# Shared state (resolves to the runner's script scope via dot-sourcing).
# TestHarnessRoot is the tests/ dir; parts use it instead of $PSScriptRoot.
$script:TestHarnessRoot = $PSScriptRoot

# TestDataDir points at the shared JSON fixtures (accounts/decisions/desires/
# risks). It can be overridden here for fixture-based special scenarios.
$script:TestDataDir = Join-Path $script:TestHarnessRoot 'TestData'

# $script:results accumulates one summary object per scenario, so the runner
# can print the trailing summary table; the runner appends, we never clear it.
$script:results = New-Object System.Collections.Generic.List[object]

# $script:phaseFailures is the scratchpad for the CURRENT phase's check lines
# ("[OK ]..."/"[FAIL]..."). Invoke-CrefoPhase clears it at phase start and
# snapshots it at phase end. Lives here (shared) because Assertions.ps1 appends
# to it and PhaseRunner.ps1 reads it.
$script:phaseFailures = New-Object System.Collections.Generic.List[string]

# Dot-source every part; each contributes its functions to this same scope.
$harnessDir = Join-Path $script:TestHarnessRoot 'TestHarness'
. (Join-Path $harnessDir 'Fixtures.ps1')
. (Join-Path $harnessDir 'MockLifecycle.ps1')
. (Join-Path $harnessDir 'ExporterRun.ps1')
. (Join-Path $harnessDir 'Readers.ps1')
. (Join-Path $harnessDir 'Assertions.ps1')
. (Join-Path $harnessDir 'PhaseRunner.ps1')