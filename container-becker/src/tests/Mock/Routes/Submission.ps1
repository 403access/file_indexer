# =============================================================================
# tests/Mock/Routes/Submission.ps1 - /api/v1/Submission/list-document and
# /api/v1/Submission/{document} endpoints.
# Dot-sourced by tests/Mock/Mock-Server.ps1.
# =============================================================================

function Invoke-MockSubmissionRoute {
    [CmdletBinding()]
    param(
        [System.Net.HttpListenerContext]$Ctx,
        [string]$Path,
        [System.Collections.Specialized.NameValueCollection]$Query
    )
    if ($Path -eq '/api/v1/Submission/list-document') {
        $script:counts.submissionList = (Get-CountValue 'submissionList') + 1
        Write-RequestLog 'GET' $Path $Query 200
        $total = $submissionDocs.Count
        Send-Json $Ctx @{
            header = @{
                currentPage  = 1
                itemsPerPage = 50
                totalItems   = [int]$total
                totalPages   = if ($total -gt 0) { 1 } else { 1 }
            }
            items  = $submissionDocs
        }
        return $true
    }
    if ($Path -match '^/api/v1/Submission/(.+)$') {
        $docName = [uri]::UnescapeDataString($Matches[1])
        $script:counts.submissionDownload = (Get-CountValue 'submissionDownload') + 1
        if ($submissionDocs.name -contains $docName) {
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
