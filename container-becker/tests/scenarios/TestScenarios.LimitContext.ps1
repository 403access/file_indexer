# =============================================================================
# TestScenarios.LimitContext.ps1 - limit-context decision scenarios.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scenarios:
#   decision-removed-refetch - active snapshot with no decision -> re-fetch
#   open-limit-pipeline      - account in open-limit-desires keeps refreshing
#   bulk-call-fallback       - decisions/desires 500 -> /risk for every account
#   free-line-from-balance   - FreeLineFromBalance switches freie Linie computation
# =============================================================================

$scenarios += @{
    Name = 'decision-removed-refetch'
    FilterTags = @('decision', 'removed')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 4102); DecisionIds = @(1014) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 4102); CsvRowCount = 2 }
        },
        @{
            Mock = @{ AccountIds = @(1014, 4102); DecisionIds = @(1014) }
            Expect = @{
                ExitCode = 0
                # 4102 still holds an active snapshot (limit 104102 / code A)
                # but has no completed decision -> the "decision removed" rule
                # re-fetches it; 1014 is unchanged and reused.
                RiskIds = @(4102)
                CsvRowCount = 2
                CsvRows = @(@{ Id = 4102; Limit = '104102,00'; Code = 'A'; Gekauft = '5102,00'; Free = '99000,00' })
                LogContains = @('completed decision removed, account previously had a limit')
            }
        }
    )
}

$scenarios += @{
    Name = 'open-limit-pipeline'
    FilterTags = @('desire', 'open')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            # 5010 has no completed decision but an in-progress desire
            Mock = @{ AccountIds = @(1014, 5010); DecisionIds = @(1014); DesireIds = @(5010) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 5010); CsvRowCount = 2 }
        },
        @{
            Mock = @{ AccountIds = @(1014, 5010); DecisionIds = @(1014); DesireIds = @(5010) }
            Expect = @{
                ExitCode = 0
                RiskIds = @(5010)      # account in the open pipeline keeps refreshing
                CsvRowCount = 2
                LogContains = @('open limit pipeline')
            }
        }
    )
}

$scenarios += @{
    Name = 'bulk-call-fallback'
    FilterTags = @('bulk', 'decisions')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Decisions500 = $true; Desires500 = $true } }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014, 2044)   # bulk context unavailable -> full /risk pass
                CsvRowCount = 2
                LogContains = @('falling back to fetching /risk for every account')
            }
        }
    )
}

$scenarios += @{
    Name = 'free-line-from-balance'
    FilterTags = @('freeline', 'balance')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0; FreeLineFromBalance = $true }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014); DecisionIds = @(1014) }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014)
                CsvRowCount = 1
                # free line = limit - balance = 100000 - 80000 = 20000 (not -Gekauft)
                CsvRows = @(@{ Id = 1014; Free = '20000,00' })
            }
        }
    )
}