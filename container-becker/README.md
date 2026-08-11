# Crefo Factoring - Limit Export (PowerShell)

Exports debtor limit data from the Crefo Factoring REST API into a CSV file:

```
Kto-Nr.;Name1;Limit;LimitKennz;Gekauft;freie Linie
1014;Irgendeine Firma Service AG;0,00;N;0,00;0,00
```

## Requirements

- PowerShell 7 (pwsh) on any OS, or Windows PowerShell 5.1+.
- API credentials (username, password, client_id, client_secret) from your CrefoFactoring representative.

## Setup

1. Install PowerShell: `winget install Microsoft.PowerShell` (Windows) or `brew install --cask powershell` (macOS).
2. Copy the example config and fill in your credentials:

   ```bash
   cp container-becker/config.example.psd1 container-becker/config.psd1
   ```

3. Edit `config.psd1`. Alternatively, set the same values via environment variables
   (these override the config file):
   `CREFO_USERNAME`, `CREFO_PASSWORD`, `CREFO_CLIENT_ID`, `CREFO_CLIENT_SECRET`,
   `CREFO_BASE_URL`, `CREFO_OBLIGO`.

   > `config.psd1` contains secrets and is git-ignored. Never commit it.

## Run

```powershell
pwsh -File container-becker/Start-CrefoExport.ps1
```

This appends rows to `container-becker/output/crefo_limits.csv` and skips accounts
that were already processed successfully on a previous run.

### Options

| Flag       | Effect                                                     |
| ---------- | ---------------------------------------------------------- |
| `-ConfigPath <path>` | Use a different config file (default: `./config.psd1`) |
| `-Reset`   | Restart from scratch: reprocess all accounts, truncate CSV |
| `-ForceToken` | Ignore the cached access token and re-authenticate      |
| `-Verbose` | Additional verbose output                                    |

Exit code is `0` on success and `1` when at least one account failed (retryable).

## What happens on each run

1. **Authenticate** (`POST /connect/token`, OAuth2 password flow). The token is cached
   in `state/crefo_token_cache.json` and reused until it expires.
2. **List accounts** (`GET /api/v1/DebitorAccounts/list-debitor`, paged).
   Fetched account IDs are merged into the persistent state, so new debtors are picked
   up and old progress is never lost.
3. **Completed limit decisions** (`GET /api/v1/last-limit-decisions`, one bulk call).
   Builds the set of accounts that actually have a limit decision. Accounts **without**
   a decision have no limit and are written straight to the CSV as `0,00;N;0,00;0,00`
   without a `/risk` request (see `UseLastLimitDecisions`). If this bulk call fails,
   the script falls back to fetching `/risk` for every account.
4. **Risk data per debtor with a decision** (`GET /api/v1/DebitorAccounts/{debitor}/risk`) -
   one request per such account. Each response is logged, applied to the CSV, and the account
   is marked `done` in the state. Every HTTP exchange (request, raw response, decoded data)
   is saved as files under `archive/` when `ArchiveRequests = true`.
5. **Retry / resume**: any account that failed keeps status `failed` and is retried on
   the next run. Existing `done` rows are not re-written, so CSV rows are never duplicated.

## Field mapping (API -> CSV)

| CSV column   | API source                                     |
| ------------ | ---------------------------------------------- |
| `Kto-Nr.`    | `id` (list-debitor) / `debtorNumber` (risk)    |
| `Name1`      | `name` (list-debitor)                          |
| `Limit`      | `limit` (risk)                                 |
| `LimitKennz` | `limitCode` (risk)                             |
| `Gekauft`    | `purchasedReceivables` (risk)                  |
| `freie Linie`| computed = `limit - Gekauft` (or `limit - balance` if `FreeLineFromBalance = true`) |

Amounts are formatted German-style (decimal comma, `;` separator, UTF-8 with BOM for Excel).

## Config options

| Key                   | Default                        | Description |
| --------------------- | ------------------------------ | ----------- |
| `BaseUrl`             | `https://api-test...`          | API base URL (test or production) |
| `Username/Password/ClientId/ClientSecret` | | Your API credentials |
| `ObligoNumber`        | `$null` (API selects lowest)   | Query one specific obligo; admins use `NMNDID-NFKDKDNR` |
| `PageSize`            | `50`                           | Items per page when listing accounts |
| `RequestDelayMs`      | `200`                          | Pause between API requests (be polite to the API) |
| `MaxRetries`          | `5`                            | Retries for transient errors (408/429/5xx/network) |
| `LogLevel`            | `INFO`                         | `DEBUG`, `INFO`, `WARN`, `ERROR` |
| `RefreshAccountList`  | `true`                         | Re-fetch the account list each run to discover new debtors |
| `FreeLineFromBalance` | `false`                        | `true` = freie Linie = limit - balance, otherwise limit - purchased |
| `UseLastLimitDecisions` | `true`                       | Skip `/risk` for accounts without a completed limit decision (written as `0,00;N;0,00;0,00`). Disable if a debtor without a decision can still have purchases. |
| `ArchiveRequests`     | `true`                         | Store every request/response/data exchange in `ArchiveDir` |
| `OutputFileName`      | `crefo_limits.csv`             | Filename in `OutputDir` |

## Generated artifacts

- `output/crefo_limits.csv` - the results.
- `state/crefo_state.json` - progress snapshot (one entry per account: id, name, status, error, updatedAt). This is what makes runs resumable. Also contains the account list snapshot in the `accounts` array.
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

## Best practices implemented

- **Logging**: timestamped, leveled log to file + console; a new log file per run.
- **Persistence**: progress, the account list, and every API request/response/data exchange are persisted to disk across runs.
- **Resumability**: completed accounts are skipped; failed ones are retried; state is saved after every single account.
- **Idempotency**: rows are appended once per account; re-running never duplicates data.
- **Security**: secrets live in git-ignored `config.psd1` or environment variables; the token cache is `chmod 600`.
- **Resilience**: retries with a configurable delay + jitter for transient errors, automatic token refresh on `401`.