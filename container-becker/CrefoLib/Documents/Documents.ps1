# =============================================================================
# CrefoLib/Documents/Documents.ps1 - document retrieval orchestration.
# Lives in the Documents submodule and is dot-sourced by CrefoLib/Documents.psm1.
# =============================================================================

function Invoke-CrefoDocuments {
    [CmdletBinding()]
    param(
        [hashtable]$Config,
        [object]$DocIndex,
        [scriptblock]$SubmissionList,
        [scriptblock]$SubmissionFetcher,
        [scriptblock]$GetDocumentDirectories,
        [scriptblock]$FolderListFactory,
        [scriptblock]$FolderFetcherFactory,
        [string]$StatePath
    )

    $script:failedDocs = 0
    $script:totalDownloaded = 0

    try {
        Write-CrefoInfo 'Listing submission documents...'
        $script:totalDownloaded += (Receive-CrefoDocumentSheet -Sheet 'submission' -List $SubmissionList -Fetcher $SubmissionFetcher -Config $Config -DocIndex $DocIndex -StatePath $StatePath)
    }
    catch {
        $script:failedDocs++
        Write-CrefoError ("Submission document retrieval failed: {0}" -f $_.Exception.Message)
        if ($_.Exception.Message -notmatch '404') { throw }
    }

    try {
        Write-CrefoInfo 'Listing available document folders...'
        $foldersResp = & $GetDocumentDirectories
        $folders = @($foldersResp.folder | Where-Object { -not [string]::IsNullOrWhiteSpace([string]$_) })
        Write-CrefoInfo ("Document folders from API: {0}." -f ($folders -join ', '))

        foreach ($folder in $folders) {
            $sheet = ConvertTo-SafeDocumentName -Value ([string]$folder)
            $folderList = & $FolderListFactory $folder
            $folderFetcher = & $FolderFetcherFactory $folder
            $script:totalDownloaded += (Receive-CrefoDocumentSheet -Sheet $sheet -List $folderList -Fetcher $folderFetcher -Config $Config -DocIndex $DocIndex -StatePath $StatePath)
        }
    }
    catch {
        $script:failedDocs++
        Write-CrefoError ("Document folder retrieval failed: {0}" -f $_.Exception.Message)
        if ($script:totalDownloaded -eq 0) { throw }
        Write-CrefoWarn 'Some folders failed, but previously downloaded documents are kept.'
    }

    Write-CrefoInfo ("Documents retrieval finished: downloaded={0} failed={1}." -f $script:totalDownloaded, $script:failedDocs)

    return [pscustomobject]@{
        Downloaded = $script:totalDownloaded
        Failed     = $script:failedDocs
    }
}
