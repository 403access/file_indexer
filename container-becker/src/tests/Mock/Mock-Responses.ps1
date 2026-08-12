# =============================================================================
# tests/Mock/Mock-Responses.ps1 - response formatters for the mock server.
# Dot-sourced by tests/Mock-CrefoApi.ps1.
# =============================================================================

function Send-Json {
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [object]$Obj,
        [int]$Status = 200
    )
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

function Send-Binary {
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Name,
        [int]$Status = 200
    )
    $body = 'content-of:' + $Name
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($body)
    $Ctx.Response.StatusCode = $Status
    $Ctx.Response.ContentType = 'application/octet-stream'
    $Ctx.Response.AddHeader('content-disposition', ('attachment; filename="{0}"' -f $Name))
    $Ctx.Response.ContentLength64 = $bytes.Length
    $Ctx.Response.OutputStream.Write($bytes, 0, $bytes.Length)
    $Ctx.Response.Close()
}

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
