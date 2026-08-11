# =============================================================================
# TestScenarios.ps1 - scenario aggregation for the Crefo scenario tests.
# Dot-sourced by Run-CrefoTests.ps1 after TestHarness.ps1.
#
# The scenario table lives in tests/scenarios/ (grouped by feature area); this
# file just assembles them into $scenarios in a stable order:
#
#   Core          - baseline happy-path sync
#   Auth          - 401 refresh / transient retry / failed-account retry
#   SyncModes     - incremental reuse / delta / probe fallback / RefreshAll
#   LimitContext  - decision removed / open pipeline / bulk fallback /
#                   free-line-from-balance
#   Operations    - refetch ranges / pagination / reset
#
# Each scenario is a hashtable:
#   Name      - scenario id (used for -Filter matching and reporting)
#   FilterTags- extra substrings/tags matched by -Filter
#   Config    - exporter config overrides for every phase of the scenario
#   Phases    - one or more phase hashtables run against the SAME runtime dir
#               (so state/db carry over between phases):
#       Mock    = @{ AccountIds; DecisionIds; DesireIds; Faults }  for the mock
#       Flags   = @{ Reset; ForceToken; RefetchRanges }  exporter run flags
#       Expect  = @{ ExitCode; RiskIds; CsvRowCount; CsvRows; CsvNotId; LogContains }
# =============================================================================

$scenarios = @()

$scenarioDir = Join-Path $PSScriptRoot 'scenarios'
. (Join-Path $scenarioDir 'TestScenarios.Core.ps1')
. (Join-Path $scenarioDir 'TestScenarios.Auth.ps1')
. (Join-Path $scenarioDir 'TestScenarios.SyncModes.ps1')
. (Join-Path $scenarioDir 'TestScenarios.LimitContext.ps1')
. (Join-Path $scenarioDir 'TestScenarios.Operations.ps1')