# =============================================================================
# tests/Mock/Mock-Data.ps1 - scenario data loading and shared state.
# Dot-sourced by tests/Mock-CrefoApi.ps1 (which owns the param block).
# =============================================================================

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

# Document scenario data (both optional; document scenarios need at least one).
$submissionDocs = @()
if ($mock.ContainsKey('SubmissionDocuments')) { $submissionDocs = @($mock['SubmissionDocuments']) }
$documentFolders = @()
if ($mock.ContainsKey('DocumentFolders')) { $documentFolders = @($mock['DocumentFolders']) }
$documentFiles = @{}
if ($mock.ContainsKey('DocumentFiles')) { $documentFiles = $mock['DocumentFiles'] }

# ------------------------------------------------------------------- state
$script:spoken401 = New-Object System.Collections.Generic.HashSet[string]
$script:risk500OnceSpoken = $false

# Counters are written as JSON after every request so the runner can assert on
# traffic shape (how many risk calls, probes, token calls etc).
$script:counts = @{ token = 0; list = 0; probe = 0; decisions = 0; desires = 0; risk = 0; submissionList = 0; submissionDownload = 0; documentsDir = 0; documentsList = 0; documentDownload = 0 }

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
