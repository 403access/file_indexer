# =============================================================================
# TestScenarios.Operations.ps1 - operational / control scenarios.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scope of this file: the knobs an operator can pull to make the exporter do
# work it would otherwise skip (-RefetchRanges, -Reset) and one volume/corner
# case (a list too big for a single page). These prove the CONTROL is what it
# claims: the switch, not the freshness heuristics, decides the fetch set.
# =============================================================================

# What we test:
#   -RefetchRanges is a targeted OVERRIDE of the incremental logic: the given
#   debitor is re-fetched even though its snapshot is fresh, while everyone
#   else is reused normally. (1014 was fetched in phase 1 and would be
#   reused in phase 2 - the range flag forces exactly the opposite.)
#
# What we expect: phase 2 fetches ONLY range '1014' (RiskIds=@(1014), NOT
# 2044, NOT both), both rows still export, and the log announces the range.
$scenarios += @{
    Name = 'refetch-ranges'
    FilterTags = @('refetch', 'range')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Flags = @{ RefetchRanges = '1014' }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014)     # only the forced range is re-fetched
                CsvRowCount = 2
                LogContains = @('Forcing /risk refetch for debtor id range(s): 1014')
            }
        }
    )
}

# What we test:
#   Account-list pagination under a small PageSize: with PageSize=2 the mock's
#   list endpoint NEVER returns all 5 accounts at once, so the exporter must
#   walk the pages to completion or it would silently export a truncated list.
#
# Note on scope: we assert the WALK is complete (all 5 exported) and the run
# succeeds; we do NOT pin the per-account /risk call pattern here - that is
# covered by the focused scenarios. (Only 3 of 5 debtors have a decision
# record; the exporter exports all 5 because the account list is the
# authoritative source.)
#
# What we expect: exit 0 and exactly 5 CSV rows - one per account, no gaps.
$scenarios += @{
    Name = 'pagination-many-accounts'
    FilterTags = @('page', 'pagination')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0; PageSize = 2 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044, 3050, 4102, 5010); DecisionIds = @(1014, 2044, 3050) }
            Expect = @{ ExitCode = 0; CsvRowCount = 5 }
        }
    )
}

# What we test:
#   -Reset reprocesses the ENTIRE account list from scratch, ignoring the
#   persisted state and all reuse rules: phase 2 runs after phase 1 stored
#   fresh snapshots for both accounts, yet BOTH are fetched again.
#
# Distinctive contrast: without the flag, phase 2 of this exact world is the
# zero-fetch case seen in incremental-reuse - so RiskIds=@(1014, 2044) here
# proves the reset switch, not a change in freshness.
#
# What we expect: phase 2 fetches both accounts again, still exits 0, still
# exports both rows, and the log confirms the reset was honoured.
$scenarios += @{
    Name = 'reset-reprocesses'
    FilterTags = @('reset')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044); CsvRowCount = 2 }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Flags = @{ Reset = $true }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014, 2044)     # reprocessed from scratch
                CsvRowCount = 2
                LogContains = @('Reset requested')
            }
        }
    )
}