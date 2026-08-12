# =============================================================================
# Mock-CrefoApi.ps1 - local stand-in for the Crefo Factoring API used by the
# scenario tests (tests/Run-CrefoTests.ps1).
#
# The mock is driven by a "mock scenario" psd1 file that describes one API
# snapshot: the debitor accounts, the completed limit decisions, the open
# limit desires and optional per-debtor /risk payloads, plus fault injection
# (500 on probe/decisions, 401-then-success per risk id, 500-once for retry
# tests). Document scenarios additionally define the submission documents and
# generic Documents folders. Responses follow the OpenAPI shapes:
#   - POST /connect/token                          -> OAuth token response
#   - GET /api/v1/DebitorAccounts/list-debitor      -> DebitorAccountDtoPagedResult
#   - GET /api/v1/last-limit-decisions              -> LastLimitDecisionDto[]
#   - GET /api/v1/open-limit-desires                -> OpenLimitDesireDto[]
#   - GET /api/v1/DebitorAccounts/{id}/risk         -> DebtorRiskInfoDto[]
#   - GET /api/v1/Submission/list-document          -> SubmissionDocumentDtoPagedResult
#   - GET /api/v1/Submission/{document}             -> binary download
#   - GET /api/v1/Documents/list-directory          -> FolderListDto
#   - GET /api/v1/Documents/{dir}/list-document     -> SubmissionDocumentDto[]
#   - GET /api/v1/Documents/{dir}/{document}        -> binary download
#
# Every exchange is appended to a JSON-lines request log (for assertions) and
# endpoint counters are persisted to a count file after each request.
# =============================================================================

param(
    [int]$Port,
    [string]$MockFile,
    [string]$RequestLog,
    [string]$CountFile,
    [string]$ReadyFile,
    [string]$StopFile
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Mock\Mock-Data.ps1')
. (Join-Path $PSScriptRoot 'Mock\Mock-Responses.ps1')
. (Join-Path $PSScriptRoot 'Mock\Mock-Server.ps1')
