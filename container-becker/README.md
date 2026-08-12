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

This appends rows to `data/output/crefo_limits.csv` and skips accounts
that were already processed successfully on a previous run.

### Options

| Flag                 | Effect                                                                                                    |
| -------------------- | --------------------------------------------------------------------------------------------------------- |
| `-ConfigPath <path>` | Use a different config file (default: `./config.psd1`)                                                    |
| `-Reset`             | Restart from scratch: reprocess all accounts, truncate CSV                                                |
| `-ForceToken`        | Ignore the cached access token and re-authenticate                                                        |
| `-RefetchRanges <r>` | Force a `/risk` refetch for these debtor ids/ranges, e.g. `"1014,1100-1200"` (overrides the config value) |
| `-Verbose`           | Additional verbose output                                                                                 |

Exit code is `0` on success and `1` when at least one account failed (retryable).

## Docs

| File                                             | Contents                                                                                                |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| [docs/usage.md](docs/usage.md)                   | What happens on each run, recovery, inspection, and document exports (`Invoke-CrefoDocuments.ps1`)      |
| [docs/flows.md](docs/flows.md)                   | Mermaid diagrams for the overall run, account discovery, risk processing, CSV rebuild, and test harness |
| [docs/reference.md](docs/reference.md)           | Field mapping, config options, and generated artifacts                                                  |
| [docs/testing.md](docs/testing.md)               | Scenario tests, how to run them, and coverage table                                                     |
| [docs/migration.md](docs/migration.md)           | One-time archive/state migration from older layouts, run-stamp migration                                |
| [docs/best-practices.md](docs/best-practices.md) | Logging, persistence, resumability, security, resilience, and memory-safe downloads                     |

## Scripts

| Script                          | Purpose                                                                                           |
| ------------------------------- | ------------------------------------------------------------------------------------------------- |
| `Start-CrefoExport.ps1`         | Main limit-export orchestrator                                                                    |
| `Invoke-CrefoDocuments.ps1`     | Binary document downloader                                                                        |
| `Rebuild-CrefoDatabase.ps1`     | Rebuild `crefo.db` + `crefo_state.json` from the archive folder                                   |
| `Inspect-CrefoDatabase.ps1`     | Read-only diagnostic queries against `crefo.db` (stats, account lookup, snapshot history)         |
| `Migrate-ArchiveRunStamp.ps1`   | Restructure an existing archive so each script run gets its own run-stamp subfolder               |
| `Reorganize-Archive.ps1`        | Migrate older archive layouts (flat, 1000-er-only buckets) into the current nested risk structure |
| `Migrate-StateFromArchive.ps1`  | Backfill `crefo_state.json` from archived risk responses (one-time)                               |
| `Show-CrefoAccountsByLimit.ps1` | List all accounts with credit limits, sorted by limit size                                        |


## Recovery & inspection

If the database or state is lost, you can rebuild both from the archive folder:

```powershell
pwsh -File Rebuild-CrefoDatabase.ps1 -ArchiveDir data/archive `
     -StateDir data/state -ConfigPath config.psd1
```

This reconstructs `data/state/crefo_state.json` and `data/state/crefo.db` from the archived API responses. A subsequent `Start-CrefoExport.ps1` run will then continue syncing gaps via the API (only refetching accounts whose decision changed or snapshot aged out).

To inspect the database:

```powershell
pwsh -File Inspect-CrefoDatabase.ps1 -DbPath data/state/crefo.db -Stats
pwsh -File Inspect-CrefoDatabase.ps1 -DbPath data/state/crefo.db -AccountId 10169
pwsh -File Inspect-CrefoDatabase.ps1 -DbPath data/state/crefo.db -AccountId 10169 -History
pwsh -File Inspect-CrefoDatabase.ps1 -DbPath data/state/crefo.db -Status done -Limit 20
```

To restructure an existing archive so each script execution gets its own run-stamp subfolder:

```powershell
pwsh -File Migrate-ArchiveRunStamp.ps1 -ArchiveDir data/archive -DryRun
pwsh -File Migrate-ArchiveRunStamp.ps1 -ArchiveDir data/archive
```

