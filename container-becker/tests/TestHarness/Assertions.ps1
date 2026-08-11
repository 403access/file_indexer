# =============================================================================
# TestHarness/Assertions.ps1 - assertion primitives. Each check appends an
# "[OK ]"/"[FAIL]" line to $script:phaseFailures; a failing line fails the
# current phase. Dot-sourced via TestHarness.ps1 into the runner's scope.
# =============================================================================

function Write-Check {
    param([string]$What, [bool]$Ok, [string]$Detail)
    $script:phaseFailures.Add($(if ($Ok) { "  [OK ] {0}" -f $What } else { "  [FAIL] {0}: {1}" -f $What, $Detail })) | Out-Null
}

function Assert-Equal {
    param([string]$What, [object]$Expected, [object]$Actual)
    $e = [array]$Expected
    $a = [array]$Actual
    Write-Check $What ($e.Count -eq $a.Count -and @(Compare-Object $e $a -SyncWindow 0).Count -eq 0) ("expected [{0}] got [{1}]" -f ($e -join ','), ($a -join ','))
}

function Assert-Contains {
    param([string]$What, [object[]]$Needle, [object[]]$Haystack, [string]$Mode = 'all')
    $found = @($Needle | Where-Object { $Haystack -contains $_ })
    $ok = if ($Mode -eq 'all') { $found.Count -eq $Needle.Count } else { $found.Count -gt 0 }
    Write-Check $What $ok ("need {0} -> found {1}" -f ($Needle -join ','), ($found -join ','))
}

function Assert-CsvRowsMatch {
    param([object[]]$Actual, [object[]]$Expected)
    foreach ($want in $Expected) {
        $line = $Actual | Where-Object { $_.'Kto-Nr.' -eq ([string]$want.Id) }
        if ($null -eq $line) {
            Write-Check ("CSV row {0} present" -f $want.Id) $false 'row missing'
            continue
        }
        if ($want.ContainsKey('Limit'))    { $a = $line.Limit.TrimEnd("`r").TrimEnd("`n").Trim(); Write-Check ("CSV row {0} Limit" -f $want.Id) ($a -eq [string]$want.Limit) ("expected {0} got {1}" -f $want.Limit, $a) }
        if ($want.ContainsKey('Code'))     { $a = $line.LimitKennz.Trim(); Write-Check ("CSV row {0} Code" -f $want.Id) ($a -eq [string]$want.Code) ("expected {0} got {1}" -f $want.Code, $a) }
        if ($want.ContainsKey('Gekauft'))  { $a = $line.Gekauft.Trim(); Write-Check ("CSV row {0} Gekauft" -f $want.Id) ($a -eq [string]$want.Gekauft) ("expected {0} got {1}" -f $want.Gekauft, $a) }
        if ($want.ContainsKey('Free'))     { $a = $line.'freie Linie'.Trim(); Write-Check ("CSV row {0} freie Linie" -f $want.Id) ($a -eq [string]$want.Free) ("expected {0} got {1}" -f $want.Free, $a) }
    }
}