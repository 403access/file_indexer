# =============================================================================
# CrefoLib/Documents/Downloads.ps1 - single-document and sheet-level download.
# Dot-sourced by CrefoLib/Documents.psm1.
# =============================================================================

function ConvertTo-SafeDocumentName {
    [CmdletBinding()]
    param([string]$Value)
    $safe = [string]$Value -replace '[^A-Za-z0-9._ -]', '_'
    $safe = $safe -replace '\.\.', '_'
    $safe = $safe -replace '^\.', '_' -replace '\.$', '_'
    if ([string]::IsNullOrWhiteSpace($safe)) { $safe = '_' }
    return $safe
}

function Receive-CrefoDocument {
    [CmdletBinding()]
    param(
        [string]$Sheet,
        [string]$Name,
        [int]$Size,
        [scriptblock]$Fetcher,
        [hashtable]$Config,
        [object]$DocIndex,
        [string]$StatePath
    )
    $safeName = ConvertTo-SafeDocumentName -Value $Name
    $sheetDir = Join-Path $Config['DocumentsDir'] $Sheet
    $target = Join-Path $sheetDir $safeName
    $key = '{0}/{1}' -f $Sheet, $Name
    $known = $DocIndex.downloaded.$key

    if ($null -ne $known -and [int]$known.size -eq $Size -and (Test-Path -LiteralPath $target)) {
        Write-CrefoInfo ("Already downloaded {0} ({1} bytes); skipping." -f $key, $Size)
        return $false
    }

    $part = Join-Path $sheetDir ($safeName + '.part')
    Remove-Item -LiteralPath $part -Force -ErrorAction SilentlyContinue
    $bytes = & $Fetcher -Name $Name -OutFile $part
    if (-not (Test-Path -LiteralPath $part)) {
        throw ("Download of {0} produced no output file." -f $key)
    }
    Move-Item -LiteralPath $part -Destination $target -Force

    $DocIndex.downloaded[$key] = [pscustomobject]@{
        sheet        = $Sheet
        name         = $Name
        size         = [int]$Size
        downloadedAt = (Get-Date).ToUniversalTime().ToString('o')
    }
    Save-DocumentIndex -Index $DocIndex -StatePath $StatePath
    Write-CrefoInfo ("Downloaded {0} ({1} bytes)." -f $key, $bytes)
    return $true
}

function Receive-CrefoDocumentSheet {
    [CmdletBinding()]
    param(
        [string]$Sheet,
        [scriptblock]$List,
        [scriptblock]$Fetcher,
        [hashtable]$Config,
        [object]$DocIndex,
        [string]$StatePath
    )
    $page = 1
    $totalCount = 0
    $downloaded = 0
    while ($true) {
        $pageResult = & $List -Page $page -PageSize 50
        $items = @($pageResult.items)
        foreach ($item in $items) {
            $name = [string]$item.name
            if ([string]::IsNullOrWhiteSpace($name)) { continue }
            $size = if ($null -ne $item.PSObject.Properties['size']) { [int]$item.size } else { 0 }
            $totalCount++
            try {
                if (Receive-CrefoDocument -Sheet $Sheet -Name $name -Size $size -Fetcher $Fetcher -Config $Config -DocIndex $DocIndex -StatePath $StatePath) {
                    $downloaded++
                }
            }
            catch {
                $script:failedDocs++
                Write-CrefoError ("Document {0}/{1} failed: {2}" -f $Sheet, $name, $_.Exception.Message)
            }
        }
        $totalPages = if ($null -ne $pageResult.header -and $null -ne $pageResult.header.totalPages) { [int]$pageResult.header.totalPages } else { $page }
        $page++
        if ($page -gt $totalPages) { break }
        if ($items.Count -eq 0) { break }
        if ($page -gt 1000) {
            Write-CrefoWarn ("Document listing for '{0}' hit the page safety cap; stopping." -f $Sheet)
            break
        }
    }
    Write-CrefoInfo ("Sheet '{0}': {1} file(s), {2} downloaded." -f $Sheet, $totalCount, $downloaded)
    return $downloaded
}
