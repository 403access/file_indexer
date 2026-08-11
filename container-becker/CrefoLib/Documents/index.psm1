# =============================================================================
# CrefoLib/Documents/index.psm1 - entry point for the Documents submodule.
# Dot-sources the implementation files and re-exports the public surface so
# callers can import a single module.
# =============================================================================
. (Join-Path $PSScriptRoot 'State.ps1')
. (Join-Path $PSScriptRoot 'Downloads.ps1')
. (Join-Path $PSScriptRoot 'Documents.ps1')
Export-ModuleMember -Function 'Get-DocumentIndex', 'Save-DocumentIndex', 'ConvertTo-SafeDocumentName', 'Receive-CrefoDocument', 'Receive-CrefoDocumentSheet', 'Invoke-CrefoDocuments'
