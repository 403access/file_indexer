# Migrating an existing archive / state (one-time)

Older versions stored risk archives differently (flat `archive/debtor-<id>-risk`,
`risks/debtor-<id>-risk`, 1000-er-only buckets, or flat underscore buckets like
`risks_<outer>-<outerEnd>_<inner>-<innerEnd>` produced by an older `ApiArchive` that
flattened the category path). Two tools bring everything to the current layout:

1. **Reorganize the archive folders** (idempotent; preview first):
   ```zsh
   pwsh -File container-becker/Reorganize-Archive.ps1 -ArchiveDir container-becker/archive -DryRun
   pwsh -File container-becker/Reorganize-Archive.ps1 -ArchiveDir container-becker/archive
   ```
   Every `debtor-<id>-risk` folder is moved to
   `archive/risks/<outer-range>/<inner-range>/debtor-<id>-risk/`; the now-empty legacy
   bucket folders are removed.

2. **Backfill account snapshots from the archive** so the next run reuses them instead of
   re-fetching `/risk` for the whole debtor book. Accounts exported before the daily-sync
   snapshot feature carry no stored risk data, so without this the first run would re-fetch
   everything once. Preview first:
   ```zsh
   pwsh -File container-becker/Migrate-StateFromArchive.ps1 -ArchiveDir container-becker/archive -DryRun
   pwsh -File container-becker/Migrate-StateFromArchive.ps1 -ArchiveDir container-becker/archive
   ```
   A timestamped backup of `state/crefo_state.json` is written before any change. Accounts
   with no archived risk are left for a normal `/risk` fetch or the N short-circuit.
