# =============================================================================
# CrefoApi.psm1 - OAuth2 authentication and REST calls against the Crefo
# Factoring API. Split across feature files by category:
#   CrefoApi/Auth.ps1      OAuth2 token retrieval + disk cache (Get-CrefoAccessToken)
#   CrefoApi/Request.ps1   HTTP plumbing: status classification, retry/backoff,
#                          one-shot 401 refresh, exchange archiving
#                          (Invoke-CrefoApi, Get-HttpStatusCode)
#   CrefoApi/Accounts.ps1  debitor account list discovery + delta walk
#                          (Get-CrefoAccounts, Get-CrefoDebtorListStats)
#   CrefoApi/Debtors.ps1   risk / limit data per debtor and the cheap bulk
#                          limit-context endpoints
#                          (Get-CrefoDebtorRisk, Get-CrefoLastLimitDecisions,
#                           Get-CrefoOpenLimitDesires)
# =============================================================================

# Dot-source the feature files in dependency order: plumbing first (token and
# account/debtor endpoints all go through Invoke-CrefoApi), then the endpoints.
. (Join-Path $PSScriptRoot 'CrefoApi\Request.ps1')
. (Join-Path $PSScriptRoot 'CrefoApi\Auth.ps1')
. (Join-Path $PSScriptRoot 'CrefoApi\Accounts.ps1')
. (Join-Path $PSScriptRoot 'CrefoApi\Debtors.ps1')

Export-ModuleMember -Function 'Get-CrefoAccessToken', 'Invoke-CrefoApi', 'Get-CrefoAccounts', 'Get-CrefoDebtorListStats', 'Get-CrefoDebtorRisk', 'Get-CrefoLastLimitDecisions', 'Get-CrefoOpenLimitDesires'