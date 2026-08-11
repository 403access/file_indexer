# =============================================================================
# TestScenarios.Core.ps1 - baseline scenarios (happy-path sync).
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scenario(s):
#   fresh-sync - first run fetches /risk for every account
# =============================================================================

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