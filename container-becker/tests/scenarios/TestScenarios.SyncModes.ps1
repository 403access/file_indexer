# =============================================================================
# TestScenarios.SyncModes.ps1 - refresh / sync-mode behaviour.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scenarios:
#   incremental-reuse     - unchanged accounts cost zero /risk calls
#   delta-new-account     - probe detects growth; only the new debtor is fetched
#   probe-failure-fallback- probe 500 -> safe full-list sync
#   refresh-all-mode      - SyncMode=RefreshAll refetches all contexts each run
# =============================================================================

$scenarios += @{
    Name = 'incremental-reuse'
    FilterTags = @('reuse', 'incremental')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044); CsvRowCount = 2 }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{
                ExitCode = 0
                RiskIds = @()               # unchanged -> zero /risk calls
                CsvRowCount = 2
                LogContains = @('Debtor list unchanged')
            }
        }
    )
}

$scenarios += @{
    Name = 'delta-new-account'
    FilterTags = @('delta')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044, 3050); DecisionIds = @(1014, 2044, 3050) }
            Expect = @{
                ExitCode = 0
                RiskIds = @(3050)            # only the new debtor gets /risk
                CsvRowCount = 3
                LogContains = @('Debtor list grew by 1', 'Delta from account list: 1 new account')
            }
        }
    )
}

$scenarios += @{
    Name = 'probe-failure-fallback'
    FilterTags = @('probe')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0; MaxRetries = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Probe500 = $true } }
            Expect = @{
                ExitCode = 0
                RiskIds = @()                # accounts already processed -> reused
                CsvRowCount = 2
                LogContains = @('falling back to full list sync')
            }
        }
    )
}

$scenarios += @{
    Name = 'refresh-all-mode'
    FilterTags = @('refreshall', 'refresh')
    Config = @{ SyncMode = 'RefreshAll'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{ ExitCode = 0; RiskIds = @(1014, 2044) }  # RefreshAll: always both
        }
    )
}