# =============================================================================
# CrefoLib/Documents.psm1 - loader for the Documents submodule.
# Dot-sources the implementation files and re-exports the public surface so
# callers can import a single module.
# =============================================================================
. (Join-Path $PSScriptRoot 'Documents\State.ps1')
. (Join-Path $PSScriptRoot 'Documents\Downloads.ps1')
. (Join-Path $PSScriptRoot 'Documents\Documents.ps1')
Export-ModuleMember -Function 'Get-DocumentIndex', 'Save-DocumentIndex', 'ConvertTo-SafeDocumentName', 'Receive-CrefoDocument', 'Receive-CrefoDocumentSheet', 'Invoke-CrefoDocuments'
