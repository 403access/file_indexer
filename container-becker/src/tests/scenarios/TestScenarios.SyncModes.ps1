# =============================================================================
# TestScenarios.SyncModes.ps1 - refresh / sync-mode behaviour.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scope of this file: HOW the exporter decides which accounts need /risk at
# all. The four scenarios pin down the states of the incremental decision:
# nothing changed / list grew / probe channel broken / "always refresh".
# The first three run under SyncMode=Incremental and are all two-phase
# (phase 1 = seed the state, phase 2 = the run whose behaviour we assert).
# =============================================================================

# What we test:
#   Incremental sync on an UNCHANGED list must fetch /risk for ZERO accounts:
#   every snapshot is fresh, the previously exported rows still describe the
#   debtors, so re-fetching would be pure waste. The second run re-exports the
#   rows from state, not from the API.
#
# This is the core efficiency guarantee - contrast with refresh-all-mode,
# which deliberately does the opposite.
#
# What we expect: phase 2 RiskIds=@() (no /risk calls at all!) while still
# exiting 0 and exporting both rows.
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

# What we test:
#   When the account list GROWS, the exporter must detect the delta and fetch
#   /risk ONLY for the newly added debtor - existing debtors stay reused.
#
# How it is triggered: phase 2 simply serves a third account (3050) that was
# absent in phase 1; the mock's account-list endpoint is the probe.
#
# What we expect: phase 2 RiskIds=@(3050) only (the new debtor - not a full
# resync), CsvRowCount=3, and log lines confirming the delta was seen both
# generically and in the account-list diff.
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

# What we test:
#   A FAILING account-list probe (the call that would tell us "nothing
#   changed") must not abort the run or, worse, silently skip work. The
#   exporter falls back to a safe FULL-LIST sync, which in one phase simply
#   re-validates the already-fresh accounts.
#
# How it is triggered: MaxRetries=0 (so the probe fails immediately instead of
# retrying into a passing state) + Faults Probe500=true. Phase 2's account
# list is unchanged from phase 1, so the expected fetch set stays empty.
#
# What we expect: exit 0 (a probe hiccup is recoverable), Zero new fetches for
# the known-fresh accounts, and the log line documenting the fallback branch.
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

# What we test:
#   SyncMode=RefreshAll explicitly overrides the incremental "reuse what is
#   fresh" logic: EVERY run in this mode must fetch /risk for every account,
#   even if every snapshot is brand new.
#
# This is the mirror image of incremental-reuse (unchanged list + empty fetch
# sets) and keeps the two modes honest: the config switch demonstrably changes
# the observable call pattern, it is not just a cosmetic log line.
#
# What we expect: phase 2 fetches BOTH accounts again, exactly like phase 1.
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