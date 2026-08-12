# =============================================================================
# TestHarness/Readers.ps1 - CSV / mock request-log / count-file readers.
# Dot-sourced via TestHarness.ps1 into the runner's scope.
#
# These three are the observation points of a phase: the exporter writes the
# CSV + a timestamped log, the mock writes the request-log and count file. The
# expectations in PhaseRunner.ps1 read all of them through these helpers so the
# assertions never touch file formats directly.
#
# Defensive defaults: every helper tolerates a missing/empty file by returning
# an empty result rather than throwing, so a phase that produced no output at
# all still yields a clean "[FAIL] ... 0 rows" instead of a crash.
# =============================================================================

# Reads the ;-separated CSV into row objects (Import-Csv is column-aware, so
# this works even when the header row's names contain special characters).
function Read-CsvRows {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return @() }
    return @(Import-Csv -LiteralPath $Path -Delimiter ';')
}

# Reads the mock's JSON-lines request log. Each line is one request record
# (method, path, account, timestamps, ...) that the mock appends while it runs;
# risk-fetch expectations derive their debitor-id list from the /risk rows.
# Malformed lines (e.g. a torn write from a concurrent append) are skipped
# rather than aborting the whole phase.
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

# Reads one counter value from the mock's count file (a small JSON object the
# mock updates per endpoint hit, used for "how often did endpoint X fire" style
# assertions). Missing file or missing key both mean "never hit" -> 0.
function Get-CountValue {
    param([string]$Path, [string]$Key)
    if (-not (Test-Path -LiteralPath $Path)) { return 0 }
    $c = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($null -eq $c.PSObject.Properties[$Key]) { return 0 }
    return [int]$c.$Key
}