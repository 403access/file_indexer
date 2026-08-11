# =============================================================================
# TestScenarios.ps1 - the scenario table for the Crefo scenario tests.
# Dot-sourced by Run-CrefoTests.ps1 after TestHarness.ps1.
#
# $scenarios is an array of scenario hashtables:
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