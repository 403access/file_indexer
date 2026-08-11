# =============================================================================
# TestScenarios.Operations.ps1 - operational / control scenarios.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scenarios:
#   refetch-ranges          - -RefetchRanges forces /risk regardless of snapshot
#   pagination-many-accounts- multi-page account list walk
#   reset-reprocesses       - -Reset reprocesses all accounts from scratch
# =============================================================================

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