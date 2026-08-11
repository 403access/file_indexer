# =============================================================================
# CrefoApi/Documents.ps1 - document retrieval for the Crefo Factoring API.
# Two families are covered (used by Invoke-CrefoDocuments.ps1):
#
#   Submission (per selected obligo / debtor context):
#     Get-CrefoSubmissionDocuments - paginated list of submission documents
#     Get-CrefoSubmissionDocument   - download one submission document (binary)
#
#   Documents (generic folders like submissions/reminders/aob):
#     Get-CrefoDocumentDirectories  - list the available folder names
#     Get-CrefoDocumentList         - list the files inside one folder
#     Get-CrefoDocumentDownload     - download one file from a folder (binary)
#
# Document names can contain any filename characters, so every path segment is
# URL-encoded (only '/' is path syntax; the rest stays opaque to the server).
# The binary downloads route through Invoke-CrefoApiDownload so they get the
# same retry/backoff + one-shot 401 recovery as every other call.
# =============================================================================

# GET /api/v1/Submission/list-document - paginated list of the submission
# documents for the authenticated debtor context. Returns the raw paged result
# ({ header, items }); the caller walks page/totalPages. unread=false returns
# ALL documents regardless of the API's "already downloaded" read flag.
function Get-CrefoSubmissionDocuments {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [bool]$Unread = $false,             # $false = list everything (default)
        [int]$Page = 1,                     # page number (1-based)
        [int]$PageSize = 50,                # items per page
        [string]$ArchiveName = 'submission-list-document'   # archive folder label
    )
    $response = Invoke-CrefoApi -Config $Config -Method GET `
        -Path '/api/v1/Submission/list-document' `
        -AccessToken $AccessToken `
        -Query @{ unread = $Unread; page = $Page; pagesize = $PageSize } `
        -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName
    return $response
}

# GET /api/v1/Submission/{document} - downloads one submission document as a
# binary stream into $OutFile. Returns the number of bytes written.
function Get-CrefoSubmissionDocument {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [string]$Document,                  # document file name (including extension)
        [string]$OutFile,                   # destination path for the downloaded bytes
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'submission-download'   # archive folder label
    )
    $path = '/api/v1/Submission/' + [uri]::EscapeDataString($Document)
    return Invoke-CrefoApiDownload -Config $Config -Path $path -AccessToken $AccessToken `
        -OutFile $OutFile -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName `
        -ArchiveCategory 'documents/submission'
}

# GET /api/v1/Documents/list-directory - lists the folder names available under
# the user's document directory (e.g. submissions, reminders, aob). Returns the
# FolderListDto ({ folder: [...] }).
function Get-CrefoDocumentDirectories {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'documents-list-directory'   # archive folder label
    )
    $response = Invoke-CrefoApi -Config $Config -Method GET `
        -Path '/api/v1/Documents/list-directory' `
        -AccessToken $AccessToken -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName
    return $response
}

# GET /api/v1/Documents/{directory}/list-document - lists the files inside one
# folder. Returns a normalized { header, items } object. The OpenAPI description
# example shows a bare DTO array for this endpoint (no header), but keep the
# paging shape detectable so { header, items } responses still work.
function Get-CrefoDocumentList {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [string]$Directory,                 # folder name, e.g. 'reminders'
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [bool]$Unread = $false,             # $false = list everything (default)
        [int]$Page = 1,                     # page number (1-based)
        [int]$PageSize = 50,                # items per page
        [string]$ArchiveName = 'documents-list-document'   # archive folder label
    )
    $path = '/api/v1/Documents/' + [uri]::EscapeDataString($Directory) + '/list-document'
    $response = Invoke-CrefoApi -Config $Config -Method GET -Path $path `
        -AccessToken $AccessToken `
        -Query @{ unread = $Unread; page = $Page; pagesize = $PageSize } `
        -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName
    if ($response -is [array]) {
        # Bare-array shape from the description example: no paging envelope.
        return [pscustomobject]@{ header = $null; items = @($response) }
    }
    return $response
}

# GET /api/v1/Documents/{directory}/{document} - downloads one file from a
# folder as a binary stream into $OutFile. Returns the number of bytes written.
function Get-CrefoDocumentDownload {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [string]$Directory,                 # folder name, e.g. 'reminders'
        [string]$Document,                  # file name (including extension)
        [string]$OutFile,                   # destination path for the downloaded bytes
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'documents-download'   # archive folder label
    )
    $path = '/api/v1/Documents/' + [uri]::EscapeDataString($Directory) + '/' + [uri]::EscapeDataString($Document)
    return Invoke-CrefoApiDownload -Config $Config -Path $path -AccessToken $AccessToken `
        -OutFile $OutFile -AuthRefresher $AuthRefresher -ArchiveName $ArchiveName `
        -ArchiveCategory 'documents'
}