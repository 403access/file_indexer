# Best practices implemented

- **Logging**: timestamped, leveled log to file + console; a new log file per run.
- **Persistence**: progress, the account list, and every API request/response/data exchange are persisted to disk across runs.
- **Resumability**: completed accounts are skipped; failed ones are retried; state is saved after every single account.
- **Idempotency**: rows are appended once per account; re-running never duplicates data.
- **Security**: secrets live in git-ignored `config.psd1` or environment variables; the token cache is `chmod 600`.
- **Resilience**: retries with a configurable delay + jitter for transient errors, automatic token refresh on `401`.
- **Memory-safe downloads**: binary documents are streamed via `HttpClient` with `ResponseHeadersRead` + chunked `CopyTo()` instead of `Invoke-WebRequest -OutFile`, which returns nothing in PowerShell 7 and makes post-call status/header inspection impossible. The chunked path keeps large files off the heap.
