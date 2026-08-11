# Testing the exporter (scenario tests)

`tests/Run-CrefoTests.ps1` drives the **real** exporter (a child `pwsh` process, so exit
codes are meaningful) against a local mock of the Crefo API (`tests/Mock-CrefoApi.ps1`).
Each scenario is one run directory with one or more *phases*; a phase is one exporter run
against one mock snapshot. Mock responses are composed per phase from the shared fixtures
in `tests/TestData/` plus fault injection, so every feature can be triggered deterministically.

```powershell
pwsh -File tests/Run-CrefoTests.ps1              # run everything
pwsh -File tests/Run-CrefoTests.ps1 -Filter reuse  # only scenarios matching 'reuse'
```

Exit code `0` = all scenarios pass, `1` = at least one failed.

## Scenario coverage

| Scenario                   | Feature under test                                             |
| -------------------------- | -------------------------------------------------------------- |
| `fresh-sync`               | first run fetches `/risk` for every account                    |
| `incremental-reuse`        | unchanged accounts cost zero `/risk` calls                     |
| `delta-new-account`        | probe detects growth, only the new debtor is fetched           |
| `probe-failure-fallback`   | probe 500 → safe full-list sync                                |
| `v401-token-refresh`       | 401 on `/risk` → token refresh + retry                         |
| `transient-retry-500`      | transient 500 → retry/backoff                                  |
| `failed-account-retry`     | permanent failure → `failed`, no CSV row, retried next run     |
| `refetch-ranges`           | `-RefetchRanges` forces `/risk` regardless of the snapshot     |
| `refresh-all-mode`         | `SyncMode=RefreshAll` refetches all accounts with a context    |
| `decision-removed-refetch` | active snapshot with no decision → re-fetch                    |
| `open-limit-pipeline`      | account in `open-limit-desires` keeps refreshing               |
| `bulk-call-fallback`       | decisions/desires 500 → fall back to `/risk` for every account |
| `free-line-from-balance`   | `FreeLineFromBalance` switches `freie Linie` computation       |
| `pagination-many-accounts` | multi-page account list walk                                   |
| `reset-reprocesses`        | `-Reset` reprocesses all accounts from scratch                 |
