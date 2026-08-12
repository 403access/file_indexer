# =============================================================================
# tests/Mock/Mock-Server.ps1 - HttpListener setup and request dispatch loop.
# Dot-sources the route handlers from tests/Mock/Routes/ and tries them in
# order for every incoming request.
# =============================================================================

. (Join-Path $PSScriptRoot 'Routes\Token.ps1')
. (Join-Path $PSScriptRoot 'Routes\Accounts.ps1')
. (Join-Path $PSScriptRoot 'Routes\LimitContext.ps1')
. (Join-Path $PSScriptRoot 'Routes\Risk.ps1')
. (Join-Path $PSScriptRoot 'Routes\Submission.ps1')
. (Join-Path $PSScriptRoot 'Routes\DocumentStore.ps1')
. (Join-Path $PSScriptRoot 'Routes\NotFound.ps1')

$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add(("http://127.0.0.1:{0}/" -f $Port))
$listener.Start()
$script:port = $Port

"READY port=$script:port" | Set-Content -LiteralPath $ReadyFile -Encoding UTF8

while ($true) {
    if (Test-Path -LiteralPath $StopFile) { break }
    try { $ctx = $listener.GetContext() } catch { break }
    $req = $ctx.Request
    $path = $req.Url.AbsolutePath
    $q = $req.QueryString
    $script:mockDebtor = ''

    $handled = $false
    $handled = Invoke-MockTokenRoute -Ctx $ctx -Path $path -Query $q
    if (-not $handled) { $handled = Invoke-MockAccountsRoute -Ctx $ctx -Path $path -Query $q }
    if (-not $handled) { $handled = Invoke-MockDecisionsRoute -Ctx $ctx -Path $path -Query $q }
    if (-not $handled) { $handled = Invoke-MockDesiresRoute -Ctx $ctx -Path $path -Query $q }
    if (-not $handled) { $handled = Invoke-MockRiskRoute -Ctx $ctx -Path $path -Query $q }
    if (-not $handled) { $handled = Invoke-MockSubmissionRoute -Ctx $ctx -Path $path -Query $q }
    if (-not $handled) { $handled = Invoke-MockDocumentStoreRoute -Ctx $ctx -Path $path -Query $q }
    if (-not $handled) { Invoke-MockNotFoundRoute -Ctx $ctx -Path $path -Query $q }

    Save-Counts
}

$listener.Stop()
