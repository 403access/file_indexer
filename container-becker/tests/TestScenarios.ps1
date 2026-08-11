# =============================================================================
# TestScenarios.ps1 - scenario aggregation for the Crefo scenario tests.
# Dot-sourced by Run-CrefoTests.ps1 after TestHarness.ps1.
#
# HOW TO READ THE CATALOGUE
# -------------------------
# A scenario = ONE behavioural guarantee of Start-CrefoExport.ps1, exercised
# against the REAL exporter (child process) + the local mock API. Each file in
# scenarios/ covers a feature area; each scenario block is read like:
#
#   Mock    - the "world" handed to the mock for this phase: which accounts
#             exist, which completed decisions / open desires they have, and
#             which faults to inject. This is NOT the expectation - it is the
#             setup that makes the behaviour under test reachable.
#   Flags   - exporter command-line switches applied for the phase (-Reset,
#             -ForceToken, -RefetchRanges). Absent = no switches.
#   Expect  - the observable contract: exit code, which debitors got /risk
#             (and in which ORDER), how many/interstate CSV rows, values of
#             specific CSV cells, and lines that must appear in the log.
#
# A scenario can span MULTIPLE phases. All phases of one scenario share the
# same runtime dir, so state.json + the sqlite db carry over between runs -
# that is the mechanism by which incremental/delta/retry behaviours are proven
# (they only make sense as "second run vs first run").
#
# Files are labelled AUTH/SYNC/ECONOMIC etc. only to keep the catalogue
# scannable; the runtime treats them identically (each appends to $scenarios).
# Order below = execution order (stable, dependency-free run-to-run).
# =============================================================================

$scenarios = @()

$scenarioDir = Join-Path $PSScriptRoot 'scenarios'
. (Join-Path $scenarioDir 'TestScenarios.Core.ps1')
. (Join-Path $scenarioDir 'TestScenarios.Auth.ps1')
. (Join-Path $scenarioDir 'TestScenarios.SyncModes.ps1')
. (Join-Path $scenarioDir 'TestScenarios.LimitContext.ps1')
. (Join-Path $scenarioDir 'TestScenarios.Operations.ps1')