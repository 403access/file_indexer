# =============================================================================
# TestHarness/Readers.ps1 - CSV / mock request-log / count-file readers.
# Dot-sourced via TestHarness.ps1 into the runner's scope.
# =============================================================================

# Reads the ;-separated CSV into row objects.
function Read-CsvRows {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return @() }
    return @(Import-Csv -LiteralPath $Path -Delimiter ';')
}

# Reads the mock's JSON-lines request log.
function Read-RequestLog {
    param([string]$Path)
    $rows = @()
    if (Test-Path -LiteralPath $Path) {
        foreach ($line in Get-Content -LiteralPath $Path) {
            if ([string]::IsNullOrWhiteSpace($line)) { continue }
            try { $rows += ($line | ConvertFrom-Json) } catch { }
        }
    }
    return $rows
}

# Reads one counter value from the mock's count file.
function Get-CountValue {
    param([string]$Path, [string]$Key)
    if (-not (Test-Path -LiteralPath $Path)) { return 0 }
    $c = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($null -eq $c.PSObject.Properties[$Key]) { return 0 }
    return [int]$c.$Key
}