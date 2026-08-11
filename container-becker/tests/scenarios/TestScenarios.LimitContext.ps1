# =============================================================================
# TestScenarios.LimitContext.ps1 - limit-context decision scenarios.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scope of this file: the rules that decide when an account that WOULD be
# "reused" (fresh snapshot) must still be re-queried, plus one config-driven
# computation switch. The common thread: the exporter does not blindly trust
# a fresh snapshot - it looks at support data (completed decisions, open
# desires, bulk availability) and only re-fetches when that data says
# "something about the limit may have changed".
# =============================================================================

# What we test:
#   The "decision removed" rule: an account whose snapshot is FRESH (would be
#   reused) but which has LOST its only completed decision must be re-fetched,
#   because a limit without a backing decision is not trustworthy to export.
#
# How it is triggered: 4102 has an active snapshot from phase 1 (limit 104102
# / code A) but NO completed decision record is served in phase 2 - the mock's
# decisions list contains only 1014. 1014 keeps its decision and is reused.
#
# What we expect: RiskIds=@(4102) (1014 untouched, 4102 re-queried exactly
# once despite being "due to reuse"), the full re-fetched limit context in the
# CSV (limit, code, gekauft AND free line - the refetch must produce a
# complete row, not a partial one), and the log line that names the rule.
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

# What we test:
#   The "open limit pipeline" rule: an account that sits in an OPEN (in-
#   progress, not yet completed) limit request must keep being refreshed on
#   every run - its limit is still being decided, so a cached snapshot can go
#   stale at any time and we cannot afford to skip it.
#
# How it is triggered: 5010 has desires.json isInProgress=true (a held-open
# request) and no completed decision; 1014 is the control (decision present,
# normally reusable).
#
# What we expect: phase 2 still fetches 5010 (RiskIds=@(5010)) while 1014 is
# skipped, both rows are still exported, and the log documents the pipeline.
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

# What we test:
#   Resilience when the BULK context endpoints fail: decisions AND desires are
#   served through one bulk call that 500s in phase 2. The exporter must not
#   fail the run and must not silently drop the decision-awareness - it falls
#   back to a plain per-account /risk pass for EVERY account (the safe
#   equivalent of "know nothing special, fetch everything").
#
# Distinctive contrast: 1014/2044 were fetched in phase 1 and would normally
# be reused in phase 2; the fallback forces them to be fetched AGAIN, which is
# exactly what the RiskIds asserts.
#
# What we expect: phase 2 RiskIds=@(1014, 2044) (both re-fetched, not reused),
# exit 0, both rows, and the log line naming the fallback branch.
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

# What we test:
#   The FreeLineFromBalance config switch changes the free-line computation:
#   default = limit - purchased amount (Gekauft), switch = limit - balance.
#   For 1014: limit 100000, balance 80000 -> freie Linie 20000,00. With Gekauft
#   empty the DEFAULT path would have produced 100000,00 - so the asserted cell
#   proves the switch actually took effect.
#
# What we expect: exactly one row for 1014 whose 'freie Linie' cell is the
# balance-derived value (the only thing this scenario cares about).
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