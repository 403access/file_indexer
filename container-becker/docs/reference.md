# Reference

## Field mapping (API -> CSV)

| CSV column    | API source                                                                          |
| ------------- | ----------------------------------------------------------------------------------- |
| `Kto-Nr.`     | `id` (list-debitor) / `debtorNumber` (risk)                                         |
| `Name1`       | `name` (list-debitor)                                                               |
| `Limit`       | `limit` (risk)                                                                      |
| `LimitKennz`  | `limitCode` (risk)                                                                  |
| `Gekauft`     | `purchasedReceivables` (risk)                                                       |
| `freie Linie` | computed = `limit - Gekauft` (or `limit - balance` if `FreeLineFromBalance = true`) |

Amounts are formatted German-style (decimal comma, `;` separator, UTF-8 with BOM for Excel).

## Config options

| Key                                       | Default                      | Description                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ----------------------------------------- | ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BaseUrl`                                 | `https://api-test...`        | API base URL (test or production)                                                                                                                                                                                                                                                                                                                                                                                                         |
| `Username/Password/ClientId/ClientSecret` |                              | Your API credentials                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `ObligoNumber`                            | `$null` (API selects lowest) | Query one specific obligo; admins use `NMNDID-NFKDKDNR`                                                                                                                                                                                                                                                                                                                                                                                   |
| `PageSize`                                | `50`                         | Items per page when listing accounts                                                                                                                                                                                                                                                                                                                                                                                                      |
| `RequestDelayMs`                          | `200`                        | Pause between API requests (be polite to the API)                                                                                                                                                                                                                                                                                                                                                                                         |
| `MaxRetries`                              | `5`                          | Retries for transient errors (408/429/5xx/network)                                                                                                                                                                                                                                                                                                                                                                                        |
| `LogLevel`                                | `INFO`                       | `DEBUG`, `INFO`, `WARN`, `ERROR`                                                                                                                                                                                                                                                                                                                                                                                                          |
| `RefreshAccountList`                      | `true`                       | Re-verify the account list each run to discover new debtors. When the database already holds accounts, the production total is probed with `pageSize=0` (falling back to `1`); if it matches the known account count the full sync is skipped, otherwise only the trailing difference is fetched. Count-based probe: cannot detect renames/substitutions when the totals stay equal. Set to `false` to keep the cached list in all cases. |
| `FreeLineFromBalance`                     | `false`                      | `true` = freie Linie = limit - balance, otherwise limit - purchased                                                                                                                                                                                                                                                                                                                                                                       |
| `UseLastLimitDecisions`                   | `true`                       | Skip `/risk` for accounts with no live limit context (not in `last-limit-decisions` or `open-limit-desires`); written as `0,00;N;0,00;0,00`. Disable if a debtor without any limit context can still have purchases.                                                                                                                                                                                                                      |
| `SyncMode`                                | `Incremental`                | `Incremental` = re-fetch `/risk` only on change/new/age; `RefreshAll` = re-fetch for all accounts with a limit context every run                                                                                                                                                                                                                                                                                                          |
| `MaxAgeDays`                              | `7`                          | In `Incremental` mode, force a `/risk` re-fetch for snapshots older than this many days (`0` = no cap). Controls `Gekauft` staleness                                                                                                                                                                                                                                                                                                      |
| `RefetchRanges`                           | `''`                         | Comma list of debtor ids / id ranges that are always re-fetched, e.g. `'1014,1100-1200'`. Overrides all incremental decisions (kept fresh despite `MaxAgeDays`/limits). Can also be passed as `-RefetchRanges` on the command line.                                                                                                                                                                                                       |
| `ArchiveRequests`                         | `true`                       | Store every request/response/data exchange in `ArchiveDir`                                                                                                                                                                                                                                                                                                                                                                                |
| `OutputFileName`                          | `crefo_limits.csv`           | Filename in `OutputDir`                                                                                                                                                                                                                                                                                                                                                                                                                   |

## Generated artifacts

- `output/crefo_limits.csv` - the results.
- `state/crefo.db` - **canonical** SQLite store (source of truth) the CSV is rebuilt from:
  - `accounts` - one row per debitor (id, name, status, error, created/updated)
  - `risk_snapshots` - append-only history of every `/risk` result (and short-circuit zero rows); latest per account wins
  - `api_exchanges` - audit log of every API call (endpoint, method, url, status, elapsed, archive path)
  Access requires the `sqlite3` CLI (ships with macOS). Writes are batched into
  atomic transactions; a failed statement rolls the whole batch back.
- `state/crefo_state.json` - progress snapshot kept **alongside** the DB for rollback (id, name, status, error, updatedAt). This is what makes runs resumable. Also contains the account list snapshot in the `accounts` array. On the first run after enabled, accounts/snapshots already in this file are copied into the database.

Because `risk_snapshots` stores `fetched_at` and the account id, archive file paths are
reconstructable without extra storage: `archive/risks/<outer-range>/<inner-range>/<debtor-id>-risk/`.

- `state/crefo_token_cache.json` - cached access token (permissions restricted).
- `logs/crefo_export_<timestamp>.log` - per-run log with timestamps and levels.
- `archive/<endpoint>/<timestamp>_<seq>_request.json` / `_response.json` / `_data.json` - every API exchange stored as files:
  - `_request.json`: method, URL (incl. query), redacted headers, request body
  - `_response.json`: HTTP status, content type, elapsed ms, **raw** response body
  - `_data.json`: the decoded/pure JSON data only (no HTTP envelope - this is the "data only" store)

  Endpoints are grouped in folders (`token`, `list-debitor`, ...). All per-debtor risk calls
  are grouped under `archive/risks/<outer-range>/<inner-range>/debtor-<id>-risk/`, bucketed
  by debtor id: outer buckets in 1000-er steps, then an inner 100-er step. For example the
  debtor with id 1234 lands under `archive/risks/1000-1999/1200-1299/debtor-1234-risk/`.
  Secrets never land in the archive: the OAuth token call is stored with credentials and
  the access token redacted, and the `Authorization` header is always written as `Bearer REDACTED`.
