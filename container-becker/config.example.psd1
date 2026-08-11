# Configuration for Start-CrefoExport.ps1
#
# Copy this file to 'config.psd1' and fill in your values, or leave the
# credential fields empty and provide them as environment variables instead
# (CREFO_USERNAME, CREFO_PASSWORD, CREFO_CLIENT_ID, CREFO_CLIENT_SECRET,
#  CREFO_BASE_URL, CREFO_OBLIGO).
#
# NOTE: config.psd1 contains secrets and is excluded via .gitignore.

@{
    # API base URL. Test: https://api-test.crefo-factoring.de
    # Production: https://api.crefo-factoring.de
    BaseUrl = 'https://api-test.crefo-factoring.de'

    # Your API credentials (provided by your CrefoFactoring representative).
    Username     = ''
    Password     = ''
    ClientId     = ''
    ClientSecret = ''

    # Optional: select one obligo. Corporate users may pass a plain number;
    # administrators use the combined format 'NMNDID-NFKDKDNR'.
    # If omitted, the API selects the lowest available obligo for the token.
    ObligoNumber = $null

    # Items per page when listing debitor accounts (API default is 50).
    PageSize = 50

    # Pause (ms) between API requests to avoid hammering the service.
    RequestDelayMs = 200

    # Retry attempts for transient failures (HTTP 408/429/5xx or network errors).
    MaxRetries = 5

    # Log verbosity: DEBUG, INFO, WARN or ERROR.
    LogLevel = 'INFO'

    # Re-fetch the debitor account list on every run so newly created debtors
    # are discovered; existing progress in the state file is preserved.
    RefreshAccountList = $true

    # 'freie Linie' is not returned by the API and is computed as:
    #   false (default): freie Linie = Limit - Gekauft (purchasedReceivables)
    #   true:            freie Linie = Limit - Balance
    FreeLineFromBalance = $false

    # Skip the per-debtor /risk request for accounts that have no live limit
    # context, i.e. appear in NEITHER the completed limit decisions NOR the
    # open limit desires (they are written as 0,00 / N / 0,00 / 0,00). The
    # context comes from two bulk calls: /last-limit-decisions and
    # /open-limit-desires. Disable if a debtor without any limit context can
    # still have purchases.
    UseLastLimitDecisions = $true

    # Daily sync strategy:
    #   Incremental (default) - few requests most days; /risk is re-fetched only
    #     when an account is new/failed, its decision changed, it entered the
    #     open limit pipeline, or its snapshot is older than MaxAgeDays.
    #   RefreshAll           - like the original one-time sync: /risk for every
    #     account with a limit context on every run.
    SyncMode = 'Incremental'

    # In Incremental mode, re-fetch the /risk snapshot of every account that
    # has not been touched for at least this many days. 0 disables the cap.
    # With MaxAgeDays = 7 the whole book is re-fetched over the course of a
    # week (about one seventh of it per day).
    MaxAgeDays = 7

    # Always re-fetch /risk for these debtor ids (comma list of single ids and
    # ranges, e.g. '1014,1100-1200'). Overrides Incremental decisions. You can
    # also pass the same value on the command line:
    #   pwsh -File Start-CrefoExport.ps1 -RefetchRanges "1014,1100-1200"
    RefetchRanges = ''

    # Store every API exchange (request.json + response.json + data.json) in
    # the archive directory so calls can be audited/replayed later.
    ArchiveRequests = $true

    # Name of the result file inside the output directory.
    OutputFileName = 'crefo_limits.csv'

    # Where retrieved submission/obligo documents are downloaded (used by
    # Invoke-CrefoDocuments.ps1). Relative to this file, or absolute.
    # Layout: <DocumentsDir>/submission/..., <DocumentsDir>/<folder>/...
    DocumentsDir = 'documents'

    # Relative to the folder that contains config.psd1:
    OutputDir = 'output'
    StateDir  = 'state'
    LogDir    = 'logs'
    ArchiveDir = 'archive'
}