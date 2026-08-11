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

    # Store every API exchange (request.json + response.json + data.json) in
    # the archive directory so calls can be audited/replayed later.
    ArchiveRequests = $true

    # Name of the result file inside the output directory.
    OutputFileName = 'crefo_limits.csv'

    # Relative to the folder that contains config.psd1:
    OutputDir = 'output'
    StateDir  = 'state'
    LogDir    = 'logs'
    ArchiveDir = 'archive'
}