# Usage

## What happens on each run

1. **Authenticate** (`POST /connect/token`, OAuth2 password flow). The token is cached
   in `state/crefo_token_cache.json` and reused until it expires.
2. **Discover the account list** (`GET /api/v1/DebitorAccounts/list-debitor`).
   The list is paginated and each response carries `header.totalItems`/`totalPages`.
   - First run / empty database: the **full list** is fetched, page by page.
   - Otherwise the database is checked first (how many accounts we already know and
     the highest known id). The production list is then **probed** with the smallest
     possible request (`pageSize=0`, falling back to `1`) to read the total size.
     If the totals match, the cached list is kept and the full sync is **skipped**
     (one request instead of a page walk). If production is larger, the **difference**
     (the trailing accounts) is fetched by offset and merged, so only the new debtors
     are requested. A probe failure degrades to the safe full-list fetch.
   Fetched account IDs are merged into the persistent state, so new debtors are picked
   up and old progress is never lost.
3. **Limit-workflow bulk calls** (`GET /api/v1/last-limit-decisions` + `GET /api/v1/open-limit-desires`,
   one call each). Their union is the set of accounts with a live limit context. Accounts in
   **neither** list have no limit and are written straight to the CSV as `0,00;N;0,00;0,00`
   without a `/risk` request. If these bulk calls fail, the script falls back to fetching
   `/risk` for every account.
4. **Risk data per debtor with a limit context** (`GET /api/v1/DebitorAccounts/{debitor}/risk`) -
   the stored snapshot decides whether a call is needed (see `SyncMode`/`MaxAgeDays`).
   Debtors listed in `RefetchRanges` (or `-RefetchRanges`) are always re-fetched, overriding
   that decision. Each call is archived under `archive/`, the response is stored in the account
   state, and the account is marked `done`.
5. **CSV rebuild** - the complete CSV is re-written every run (header + all rows, stable order)
   from the stored snapshots, so the file is always complete and unchanged accounts cost zero requests.
6. **Document downloads** (`GET /api/v1/Submission/{document}` and `GET /api/v1/Documents/{folder}/{document}`).
   Binary files are streamed straight to disk via `HttpClient` (response headers are read
   first, then the body is copied in chunks so a large PDF or invoice never sits entirely
   in memory). Only metadata (status, content-type, file size) is archived; the raw binary
   body is never persisted. The same retry/backoff and one-shot 401 recovery used for
   JSON endpoints applies here too. Run `Invoke-CrefoDocuments.ps1` to export the
   submission and generic-document inventories.
7. **Retry / resume**: any account that failed keeps status `failed` and is retried on
   the next run. Failed accounts are omitted from the rebuilt CSV until a later run succeeds.

## Recovery & inspection

If the database or state is lost, you can rebuild both from the archive folder:

```powershell
pwsh -File container-becker/Rebuild-CrefoDatabase.ps1 `
     -ArchiveDir "/Users/olivermolnar/Downloads/container-becker/container-becker/archive" `
     -StateDir "/Users/olivermolnar/Downloads/container-becker/container-becker/state" `
     -ConfigPath "/Users/olivermolnar/Downloads/container-becker/container-becker/config.psd1"
```

This reconstructs `crefo_state.json` and `crefo.db` from the archived API responses. A subsequent `Start-CrefoExport.ps1` run will then continue syncing gaps via the API (only refetching accounts whose decision changed or snapshot aged out).

To inspect the database:

```powershell
pwsh -File container-becker/Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -Stats
pwsh -File container-becker/Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -AccountId 10169
pwsh -File container-becker/Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -AccountId 10169 -History
pwsh -File container-becker/Inspect-CrefoDatabase.ps1 -DbPath state/crefo.db -Status done -Limit 20
```

To restructure an existing archive so each script execution gets its own run-stamp subfolder:

```powershell
pwsh -File container-becker/Migrate-ArchiveRunStamp.ps1 -ArchiveDir container-becker/archive -DryRun
pwsh -File container-becker/Migrate-ArchiveRunStamp.ps1 -ArchiveDir container-becker/archive
```

## Document exports (`Invoke-CrefoDocuments.ps1`)

`Invoke-CrefoDocuments.ps1` handles the binary document endpoints separately from
the limit-export flow. It supports two inventories:

- **Submission documents** (`/api/v1/Submission/list-document` + `/api/v1/Submission/{name}`)
- **Generic documents** (`/api/v1/Documents/list-directory` + `/api/v1/Documents/{folder}/list-document` + `/api/v1/Documents/{folder}/{name}`)

```powershell
pwsh -File container-becker/Invoke-CrefoDocuments.ps1
```

Options:

| Flag                 | Effect                                                              |
| -------------------- | ------------------------------------------------------------------- |
| `-ConfigPath <path>` | Use a different config file (default: `./config.psd1`)              |
| `-OutputDir <path>`  | Destination folder for downloaded binaries (default: `./documents`) |
| `-Verbose`           | Additional verbose output                                           |

Each downloaded file is written to `<OutputDir>/<category>/<filename>`. The archive
records the HTTP status, content-type, and byte size for every request, but the raw
binary body is never written to the archive so the folder stays small.
