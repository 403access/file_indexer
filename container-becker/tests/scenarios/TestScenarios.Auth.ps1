# =============================================================================
# TestScenarios.Auth.ps1 - authentication & resilience scenarios.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scenarios:
#   v401-token-refresh      - 401 on /risk -> token refresh + retry
#   transient-retry-500     - transient 500 -> retry/backoff
#   failed-account-retry    - permanent failure -> 'failed', no CSV row, retried
# =============================================================================

$scenarios += @{
    Name = 'v401-token-refresh'
    FilterTags = @('401', 'refresh')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Risk401OnceIds = @(1014) } }
            Expect = @{
                ExitCode = 0
                # 1014 was 401-once (2 attempts) + 2044 = 3 /risk calls total
                RiskIds = @(1014, 1014, 2044)
                CsvRowCount = 2
                LogContains = @('HTTP 401', 'refreshing access token')
            }
        }
    )
}

$scenarios += @{
    Name = 'transient-retry-500'
    FilterTags = @('retry')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0; MaxRetries = 2 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Risk500OnceId = 1014 } }
            Expect = @{
                ExitCode = 0
                RiskIds = @(1014, 1014, 2044)  # 1014 failed once then succeeded
                CsvRowCount = 2
                LogContains = @('Transient error HTTP 500')
            }
        }
    )
}

$scenarios += @{
    Name = 'failed-account-retry'
    FilterTags = @('failed', 'retry')
    Config = @{ SyncMode = 'Incremental'; MaxAgeDays = 0 }
    Phases = @(
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044); Faults = @{ Risk500Ids = @(2044) } }
            Expect = @{
                ExitCode = 1          # 2044 permanently fails -> run fails
                CsvNotId = 2044       # failed accounts produce no CSV row
                LogContains = @('Debitor 2044 failed')
            }
        },
        @{
            Mock = @{ AccountIds = @(1014, 2044); DecisionIds = @(1014, 2044) }
            Expect = @{
                ExitCode = 0          # continues after the failed account recovers
                RiskIds = @(2044)     # failed account is retried
                CsvRowCount = 2
                LogContains = @('Debtor list unchanged')
            }
        }
    )
}