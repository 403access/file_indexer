# =============================================================================
# CrefoLib/Documents/State.ps1 - document download index persistence.
# Dot-sourced by CrefoLib/Documents/index.psm1.
# =============================================================================

function Get-DocumentIndex {
    [CmdletBinding()]
    param([string]$StatePath)
    if (Test-Path -LiteralPath $StatePath) {
        try {
            $index = Get-Content -LiteralPath $StatePath -Raw -Encoding UTF8 | ConvertFrom-Json
            if ($null -eq $index.downloaded) {
                $index | Add-Member -NotePropertyName downloaded -NotePropertyValue @{} -Force
            }
            else {
                $table = @{}
                foreach ($p in $index.downloaded.PSObject.Properties) { $table[$p.Name] = $p.Value }
                $index.downloaded = $table
            }
            return $index
        }
        catch {
            Write-CrefoWarn ("Document index '{0}' could not be read, starting fresh: {1}" -f $StatePath, $_.Exception.Message)
        }
    }
    return [PSCustomObject]@{ version = 1; updatedAt = $null; downloaded = @{} }
}

function Save-DocumentIndex {
    [CmdletBinding()]
    param([object]$Index, [string]$StatePath)
    $Index.updatedAt = (Get-Date).ToUniversalTime().ToString('o')
    $Index | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $StatePath -Encoding UTF8
}
