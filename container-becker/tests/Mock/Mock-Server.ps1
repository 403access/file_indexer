# =============================================================================
# tests/Mock/Mock-Server.ps1 - HTTP listener and request routing.
# Dot-sourced by tests/Mock-CrefoApi.ps1 after Mock-Data.ps1 and
# Mock-Responses.ps1 so all data, counters, and response helpers are in scope.
# =============================================================================

$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add(("http://127.0.0.1:{0}/" -f $Port))
$listener.Start()
$script:port = $Port

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
            $isProbe = ($pageSize -le 1)
            if ($isProbe) {
                $script:counts.probe = (Get-CountValue 'probe') + 1
                if ($faults['Probe500']) {
                    Write-RequestLog 'GET' $path $q 500
                    Send-Json $ctx @{ error = 'probe fault (simulated)' } 500
                }
                else {
                    Write-RequestLog 'GET' $path $q 200
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
                $script:spoken401.Add([string]$id) | Out-Null
                Write-RequestLog 'GET' $path $q 401 $debitor
                Send-Json $ctx @{ error = 'unauthorized (simulated)' } 401
            }
            elseif ($id -eq [int]$faults['Risk500OnceId'] -and -not $script:risk500OnceSpoken) {
                $script:risk500OnceSpoken = $true
                Write-RequestLog 'GET' $path $q 500 $debitor
                Send-Json $ctx @{ error = 'transient fault (simulated)' } 500
            }
            else {
                Write-RequestLog 'GET' $path $q 200 $debitor
                Send-Json $ctx @(Get-RiskForId -Id $id)
            }
        }
        # ------------------------------------------------- submission list
        elseif ($path -eq '/api/v1/Submission/list-document') {
            $script:counts.submissionList = (Get-CountValue 'submissionList') + 1
            Write-RequestLog 'GET' $path $q 200
            $total = $submissionDocs.Count
            Send-Json $ctx @{
                header = @{
                    currentPage  = 1
                    itemsPerPage = 50
                    totalItems   = [int]$total
                    totalPages   = if ($total -gt 0) { 1 } else { 1 }
                }
                items  = $submissionDocs
            }
        }
        # --------------------------------------------- submission download
        elseif ($path -match '^/api/v1/Submission/(.+)$') {
            $docName = [uri]::UnescapeDataString($Matches[1])
            $script:counts.submissionDownload = (Get-CountValue 'submissionDownload') + 1
            if ($submissionDocs.name -contains $docName) {
                Write-RequestLog 'GET' $path $q 200
                Send-Binary $ctx $docName
            }
            else {
                Write-RequestLog 'GET' $path $q 404
                $ctx.Response.StatusCode = 404
                $ctx.Response.Close()
            }
        }
        # ------------------------------------------------ document folders
        elseif ($path -eq '/api/v1/Documents/list-directory') {
            $script:counts.documentsDir = (Get-CountValue 'documentsDir') + 1
            Write-RequestLog 'GET' $path $q 200
            Send-Json $ctx @{ folder = @($documentFolders) }
        }
        # ------------------------------------------------- document listing
        elseif ($path -match '^/api/v1/Documents/([^/]+)/list-document$') {
            $dirName = [uri]::UnescapeDataString($Matches[1])
            $script:counts.documentsList = (Get-CountValue 'documentsList') + 1
            if ($documentFiles.ContainsKey($dirName)) {
                Write-RequestLog 'GET' $path $q 200
                Send-Json $ctx @($documentFiles[$dirName])
            }
            else {
                Write-RequestLog 'GET' $path $q 404
                $ctx.Response.StatusCode = 404
                $ctx.Response.Close()
            }
        }
        # -------------------------------------------------- document download
        elseif ($path -match '^/api/v1/Documents/([^/]+)/([^/]+)$') {
            $dirName = [uri]::UnescapeDataString($Matches[1])
            $docName = [uri]::UnescapeDataString($Matches[2])
            $script:counts.documentDownload = (Get-CountValue 'documentDownload') + 1
            $inFolder = $false
            if ($documentFiles.ContainsKey($dirName)) {
                $inFolder = (@($documentFiles[$dirName]).name -contains $docName)
            }
            if ($inFolder) {
                Write-RequestLog 'GET' $path $q 200
                Send-Binary $ctx $docName
            }
            else {
                Write-RequestLog 'GET' $path $q 404
                $ctx.Response.StatusCode = 404
                $ctx.Response.Close()
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
