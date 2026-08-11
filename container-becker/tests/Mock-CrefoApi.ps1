# =============================================================================
# Mock-CrefoApi.ps1 - local stand-in for the Crefo Factoring API used by the
# scenario tests (tests/Run-CrefoTests.ps1).
#
# The mock is driven by a "mock scenario" psd1 file that describes one API
# snapshot: the debitor accounts, the completed limit decisions, the open
# limit desires and optional per-debtor /risk payloads, plus fault injection
# (500 on probe/decisions, 401-then-success per risk id, 500-once for retry
# tests). Responses follow the OpenAPI shapes documented in api.md/swagger:
#   - POST /connect/token                  -> OAuth token response
#   - GET /api/v1/DebitorAccounts/list-debitor -> DebitorAccountDtoPagedResult
#   - GET /api/v1/last-limit-decisions     -> LastLimitDecisionDto[]
#   - GET /api/v1/open-limit-desires       -> OpenLimitDesireDto[]
#   - GET /api/v1/DebitorAccounts/{id}/risk -> DebtorRiskInfoDto[]
#
# Every exchange is appended to a JSON-lines request log (for assertions) and
# endpoint counters are persisted to a count file after each request.
# =============================================================================

param(
    [int]$Port,            # TCP port to listen on (runner picks a free one)
    [string]$MockFile,     # path to the mock scenario psd1 (data + faults)
    [string]$RequestLog,   # JSON-lines log of all requests (one object per line)
    [string]$CountFile,    # JSON counters: token/list/probe/decisions/desires/risk
    [string]$ReadyFile,    # written with 'READY' when the listener is up
    [string]$StopFile      # when this file appears, the server shuts down
)

$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------- scenario
if ([System.IO.Path]::GetExtension($MockFile) -eq '.json') {
    $mock = Get-Content -LiteralPath $MockFile -Raw | ConvertFrom-Json -AsHashtable
}
else {
    $mock = Import-PowerShellDataFile -LiteralPath $MockFile
}
$accounts = @($mock['Accounts'])
if ($accounts.Count -eq 0) { throw 'Mock scenario must define at least one Accounts entry.' }
$decisions = @($mock['Decisions'])
$desires = @($mock['Desires'])
$riskById = @{}
if ($mock.ContainsKey('Risk')) {
    foreach ($entry in $mock['Risk'].GetEnumerator()) {
        $riskById[[string]$entry.Key] = $entry.Value
    }
}
$faults = @{}
if ($mock.ContainsKey('Faults')) { $faults = $mock['Faults'] }

# ------------------------------------------------------------------- state
$script:spoken401 = New-Object System.Collections.Generic.HashSet[string]
$script:risk500OnceSpoken = $false
$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add(("http://127.0.0.1:{0}/" -f $Port))
$listener.Start()
$script:port = $Port

# Counters are written as JSON after every request so the runner can assert on
# traffic shape (how many risk calls, probes, token calls etc).
$script:counts = @{ token = 0; list = 0; probe = 0; decisions = 0; desires = 0; risk = 0 }

function Get-CountValue {
    param([string]$Name)
    if ($script:counts.ContainsKey($Name)) { return $script:counts[$Name] }
    return 0
}

# Appends one JSON line to the request log: { t, method, path, query, status, debitor }
function Write-RequestLog {
    param(
        [string]$Method, [string]$Path, [System.Collections.Specialized.NameValueCollection]$Query,
        [int]$Status, [string]$Debitor = ''
    )
    $entry = [ordered]@{
        t       = [DateTime]::UtcNow.ToString('o')
        method  = $Method
        path    = $Path
        status  = $Status
        debitor = $Debitor
    }
    if ($null -ne $Query) {
        $entry['query'] = ([string[]]($Query.AllKeys | ForEach-Object { "$_=$($Query[$_])" })) -join '&'
    }
    Add-Content -LiteralPath $RequestLog -Value ($entry | ConvertTo-Json -Compress) -Encoding UTF8
}

function Save-Counts {
    $script:counts | ConvertTo-Json -Compress | Set-Content -LiteralPath $CountFile -Encoding UTF8
}

function Send-Json {
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [object]$Obj,
        [int]$Status = 200
    )
    # Normalize an empty/null body or an empty collection to a JSON array so
    # the shape stays stable for the client (endpoints return DTO arrays).
    $json = $null
    if ($null -eq $Obj) { $json = 'null' }
    else {
        $asArray = @($Obj)
        if ($asArray.Count -eq 0) { $json = '[]' }
        else { $json = $asArray | ConvertTo-Json -Depth 20 -Compress }
    }
    if ([string]::IsNullOrWhiteSpace($json)) { $json = 'null' }
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
    $Ctx.Response.StatusCode = $Status
    $Ctx.Response.ContentType = 'application/json'
    $Ctx.Response.ContentLength64 = $bytes.Length
    $Ctx.Response.OutputStream.Write($bytes, 0, $bytes.Length)
    $Ctx.Response.Close()
}

# Default /risk body derived from the id when the scenario gives no explicit
# Risk entry (mirrors the realistic shape: limit ~ 100k, a fraction purchased).
function Get-RiskForId {
    param([int]$Id)
    if ($riskById.ContainsKey([string]$Id)) { return $riskById[[string]$Id] }
    $code = if (($Id % 2) -eq 0) { 'A' } else { 'B' }
    return [ordered]@{
        debtorNumber          = $Id
        companyName           = "Debitor $Id"
        currencyDescription   = 'EUR'
        limit                 = [double](100000 + $Id)
        purchasedReceivables  = [double](1000 + $Id)
        balance               = [double](50 + $Id)
        limitCode             = $code
    }
}

"READY port=$script:port" | Set-Content -LiteralPath $ReadyFile -Encoding UTF8

# ------------------------------------------------------------------- server
while ($true) {
    if (Test-Path -LiteralPath $StopFile) { break }
    try { $ctx = $listener.GetContext() } catch { break }
    $req = $ctx.Request
    $path = $req.Url.AbsolutePath
    $q = $req.QueryString
    $debitor = ''

    try {
        # -------------------------------------------------------- token
        if ($path -eq '/connect/token') {
            $script:counts.token = (Get-CountValue 'token') + 1
            Send-Json $ctx @{
                access_token = ("mock-token-$([guid]::NewGuid().ToString('N'))")
                token_type   = 'Bearer'
                expires_in   = 3600
                isCorporateLogin = $false
            }
        }
        # -------------------------------------------------------- list
        elseif ($path -eq '/api/v1/DebitorAccounts/list-debitor') {
            $pageSize = if ($null -eq $q['pagesize']) { 50 } else { [int]$q['pagesize'] }
            $page = if ($null -eq $q['page']) { 1 } else { [int]$q['page'] }
            $isProbe = ($pageSize -le 1)   # probe uses pageSize=0 -> falls back to 1
            if ($isProbe) {
                $script:counts.probe = (Get-CountValue 'probe') + 1
                if ($faults['Probe500']) {
                    Write-RequestLog 'GET' $path $q 500
                    Send-Json $ctx @{ error = 'probe fault (simulated)' } 500
                }
                else {
                    Write-RequestLog 'GET' $path $q 200
                    # Probe pages return just the header so the client can read
                    # header.totalItems; no items are required.
                    Send-Json $ctx @{
                        header = @{
                            currentPage  = 1
                            itemsPerPage = $pageSize
                            totalItems   = [int]$accounts.Count
                            totalPages   = if ($pageSize -ge 1) { [math]::Ceiling([double]$accounts.Count / [double]$pageSize) } else { 1 }
                        }
                        items  = @()
                    }
                }
            }
            else {
                $script:counts.list = (Get-CountValue 'list') + 1
                Write-RequestLog 'GET' $path $q 200
                $totalPages = [math]::Ceiling([double]$accounts.Count / [double]$pageSize)
                $start = ($page - 1) * $pageSize
                $items = @($accounts | Select-Object -Skip $start -First $pageSize)
                Send-Json $ctx @{
                    header = @{
                        currentPage  = [int]$page
                        itemsPerPage = [int]$pageSize
                        totalItems   = [int]$accounts.Count
                        totalPages   = [int]$totalPages
                    }
                    items  = $items
                }
            }
        }
        # ---------------------------------------------------- decisions
        elseif ($path -eq '/api/v1/last-limit-decisions') {
            $script:counts.decisions = (Get-CountValue 'decisions') + 1
            if ($faults['Decisions500']) {
                Write-RequestLog 'GET' $path $q 500
                Send-Json $ctx @{ error = 'decisions fault (simulated)' } 500
            }
            else {
                Write-RequestLog 'GET' $path $q 200
                Send-Json $ctx @($decisions)
            }
        }
        # ------------------------------------------------------ desires
        elseif ($path -eq '/api/v1/open-limit-desires') {
            $script:counts.desires = (Get-CountValue 'desires') + 1
            if ($faults['Desires500']) {
                Write-RequestLog 'GET' $path $q 500
                Send-Json $ctx @{ error = 'desires fault (simulated)' } 500
            }
            else {
                Write-RequestLog 'GET' $path $q 200
                Send-Json $ctx @($desires)
            }
        }
        # -------------------------------------------------------- risk
        elseif ($path -match '^/api/v1/DebitorAccounts/(\d+)/risk$') {
            $id = [int]$Matches[1]
            $debitor = [string]$id
            $script:counts.risk = (Get-CountValue 'risk') + 1

            if ($id -in @($faults['Risk500Ids'])) {
                Write-RequestLog 'GET' $path $q 500 $debitor
                Send-Json $ctx @{ error = 'risk fault (simulated)' } 500
            }
            elseif ($id -in @($faults['Risk401OnceIds']) -and -not $script:spoken401.Contains([string]$id)) {
                # First /risk for this id returns 401; the client refreshes its
                # token and retries -> the second call succeeds. Tests the
                # one-shot 401 recovery path in Invoke-CrefoApi.
                $script:spoken401.Add([string]$id) | Out-Null
                Write-RequestLog 'GET' $path $q 401 $debitor
                Send-Json $ctx @{ error = 'unauthorized (simulated)' } 401
            }
            elseif ($id -eq [int]$faults['Risk500OnceId'] -and -not $script:risk500OnceSpoken) {
                # First call for this single id returns 500, subsequent succeed:
                # tests the retry/backoff path.
                $script:risk500OnceSpoken = $true
                Write-RequestLog 'GET' $path $q 500 $debitor
                Send-Json $ctx @{ error = 'transient fault (simulated)' } 500
            }
            else {
                Write-RequestLog 'GET' $path $q 200 $debitor
                # The API returns an array (DebtorRiskInfoDto[]); the client
                # reads the first element, so wrap the payload in an array.
                Send-Json $ctx @(Get-RiskForId -Id $id)
            }
        }
        # ------------------------------------------------------- 404s
        else {
            Write-RequestLog 'GET' $path $q 404 $debitor
            $ctx.Response.StatusCode = 404
            $ctx.Response.Close()
        }
    }
    catch {
        try {
            Write-RequestLog "$($req.HttpMethod)" $path $q 599 $debitor
            $ctx.Response.StatusCode = 599
            $ctx.Response.Close()
        }
        catch { }
    }
    Save-Counts
}

$listener.Stop()