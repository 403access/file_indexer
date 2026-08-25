# =============================================================================
# TestScenarios.Auth.ps1 - authentication & resilience scenarios.
# Dot-sourced by TestScenarios.ps1; appends to $scenarios.
#
# Scope of this file: what the exporter does when the API answers with an
# auth/channel error instead of data. The three cases differ in whether the
# failure is (a) a one-off 401 that token-refresh fixes, (b) a transient 500
# that a retry fixes, or (c) a persistent 500 that no retry fixes - and each
# has a DIFFERENT contract for the run's outcome.
# =============================================================================

# What we test:
#   A 401 on /risk (token expired mid-run) must NOT fail the account: the
#   exporter refreshes its access token and retries, and the account then
#   succeeds as if nothing happened.
#
# How it is triggered: Faults Risk401OnceIds=@(1014) makes the mock answer 401
# ONLY on the FIRST /risk call for 1014 (all later calls, and all of 2044's,
# are fine). So exactly two /risk requests for 1014 are expected - matching
# the "attempt -> 401 -> refresh -> retry" sequence.
#
# What we expect: exit 0, both rows exported, and the LOG must prove the
# mechanism (both the observed 401 and the subsequent refresh).
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

# What we test:
#   A transient 500 (one-off server hiccup) must be absorbed by the retry
#   mechanism: MaxRetries=2 configured, the account succeeds on the retry, and
#   the run still completes cleanly.
#
# How it is triggered: Faults Risk500OnceId=1014 -> 2044 never fails, 1014's
# FIRST /risk 500s and then succeeds. Distinct from the next scenario
# (failed-account-retry) which injects PERSISTENT 500s.
#
# What we expect: 1014 appears twice in the fetch order (failed attempt +
# successful retry), exit 0, both rows exported.
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

# What we test (the asymmetric pair, run -Retry + fresh run):
#   Phase 1 - when an account KEEPS 500ing past all retries it must be marked
#             'failed': the run reports failure (exit 1) and the account gets
#             NO CSV row (a wrong/empty row would poison downstream data).
#   Phase 2 - the failure must be durable enough to be RETRIED on the next run
#             (not silently skipped forever): once the API heals, the next run
#             fetches only the previously-failed account, which now succeeds.
#
# How it is triggered: phase 1 injects Risk500Ids=2044 (permanent); phase 2
# removes the fault. Both phases share the runtime dir, so the exporter's
# persisted "2044 failed" marker from phase 1 is what phase 2 retries against.
#
# What we expect: phase 1 exit 1 + no 2044 row + 'Debitor 2044 failed'; phase 2
# exit 0, RiskIds=@(2044) ONLY (1014 is fresh and reused - proving it is a
# targeted retry, not a full resync), 2 rows again, 'Debtor list unchanged'.
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