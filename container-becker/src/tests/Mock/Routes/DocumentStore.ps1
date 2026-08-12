# =============================================================================
# tests/Mock/Routes/DocumentStore.ps1 - /api/v1/Documents/* endpoints.
# Dot-sourced by tests/Mock/Mock-Server.ps1.
# =============================================================================

function Invoke-MockDocumentStoreRoute {
    [CmdletBinding()]
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Path,
        [System.Collections.Specialized.NameValueCollection]$Query
    )
    if ($Path -eq '/api/v1/Documents/list-directory') {
        $script:counts.documentsDir = (Get-CountValue 'documentsDir') + 1
        Write-RequestLog 'GET' $Path $Query 200
        Send-Json $Ctx @{ folder = @($documentFolders) }
        return $true
    }
    if ($Path -match '^/api/v1/Documents/([^/]+)/list-document$') {
        $dirName = [uri]::UnescapeDataString($Matches[1])
        $script:counts.documentsList = (Get-CountValue 'documentsList') + 1
        if ($documentFiles.ContainsKey($dirName)) {
            Write-RequestLog 'GET' $Path $Query 200
            Send-Json $Ctx @($documentFiles[$dirName])
        }
        else {
            Write-RequestLog 'GET' $Path $Query 404
            $Ctx.Response.StatusCode = 404
            $Ctx.Response.Close()
        }
        return $true
    }
    if ($Path -match '^/api/v1/Documents/([^/]+)/([^/]+)$') {
        $dirName = [uri]::UnescapeDataString($Matches[1])
        $docName = [uri]::UnescapeDataString($Matches[2])
        $script:counts.documentDownload = (Get-CountValue 'documentDownload') + 1
        $inFolder = $false
        if ($documentFiles.ContainsKey($dirName)) {
            $inFolder = (@($documentFiles[$dirName]).name -contains $docName)
        }
        if ($inFolder) {
            Write-RequestLog 'GET' $Path $Query 200
            Send-Binary $Ctx $docName
        }
        else {
            Write-RequestLog 'GET' $Path $Query 404
            $Ctx.Response.StatusCode = 404
            $Ctx.Response.Close()
        }
        return $true
    }
    return $false
}
