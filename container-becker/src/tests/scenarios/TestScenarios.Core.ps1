# =============================================================================
# TestScenarios.Core.ps1 - baseline / happy-path scenarios.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scope of this file: prove the simplest guarantee the rest of the catalogue is
# built on - on a truly first run the exporter works through the whole account
# list, fetches /risk for every account and exports every row.
# =============================================================================

# What we test:
#   A run with no prior state (fresh runtime dir) MUST fetch /risk for every
#   account in the list and export exactly one CSV row per account.
#
# Why this is the baseline: every later scenario builds on "first run == full
# fetch". This locks in that the plain happy path works at all, so any failure
# reported by another scenario can be blamed on the specific fault/setup, not
# on a broken exporter pipeline.
#
# Setup note: this scenario has no second phase and no faults, so the only
# possible outcome of a regression is a wrong RiskIds/row-count/exit-code.
$scenarios += @{
    Name = 'fresh-sync'
    FilterTags = @('fresh')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            # Two accounts, both with a completed decision. No /risk faults,
            # no state carried in -> nothing can be skipped.
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014, 2044)   # every account fetched, in list order
                CsvRowCount = 2           # one exported row per account
            }
        }
    )
}