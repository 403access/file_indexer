# =============================================================================
# TestHarness/Assertions.ps1 - assertion primitives. Each check appends an
# "[OK ]"/"[FAIL]" line to $script:phaseFailures; a failing line fails the
# current phase. Dot-sourced via TestHarness.ps1 into the runner's scope.
#
# WHY ACCUMULATE, NOT THROW
# -------------------------
# A phase asserts MANY things (exit code, ids fetched, CSV fields, log lines).
# Throwing on the first mismatch would mask every check after it and force a
# fix-repeat cycle of one assertion at a time. Instead every check writes its
# own verdict line into $script:phaseFailures, and Invoke-CrefoPhase later
# scans that list for any '[FAIL]' prefix to decide the phase verdict. The
# runner prints ALL lines, so a single failed phase shows every problem at once.
#
# The verdict lines carry no trailing newline per check; the runner joins the
# snapshot back into a block, keeping the report compact and aligned.
# =============================================================================

# Writes one verdict line. $Detail is folded into the line ONLY on failure (so
# the happy path stays clean) and answers "expected X got Y" / "missing ...".
function Write-Check {
    param([string]$What, [bool]$Ok, [string]$Detail)
    $script:phaseFailures.Add($(if ($Ok) { "  [OK ] {0}" -f $What } else { "  [FAIL] {0}: {1}" -f $What, $Detail })) | Out-Null
}

# Order-sensitive equality of two value collections (ids, counts, exit codes).
# Works for scalars too: casting to [array] makes a single value comparable to
# a one-element expected list, and an EMPTY result (the classic $null-turns-
# into-nothing pitfall) is honestly compared as "0 vs N" rather than throwing.
#
# -SyncWindow 0 forces Compare-Object to respect ORDER (default unordered);
# that matters when the expectation encodes sequence, e.g. the order in which
# the exporter fetched /risk for accounts.
function Assert-Equal {
    param([string]$What, [object]$Expected, [object]$Actual)
    $e = [array]$Expected
    $a = [array]$Actual
    Write-Check $What ($e.Count -eq $a.Count -and @(Compare-Object $e $a -SyncWindow 0).Count -eq 0) ("expected [{0}] got [{1}]" -f ($e -join ','), ($a -join ','))
}

# Subset membership of $Needle within $Haystack (used e.g. for "this exact set
# of debitors was fetched" style risk checks).
#   Mode 'all' (default): every needle must be present.
#   Mode 'any'          : at least one needle present (rare; kept for the
#                         "either A or B behaviour is acceptable" cases).
function Assert-Contains {
    param([string]$What, [object[]]$Needle, [object[]]$Haystack, [string]$Mode = 'all')
    $found = @($Needle | Where-Object { $Haystack -contains $_ })
    $ok = if ($Mode -eq 'all') { $found.Count -eq $Needle.Count } else { $found.Count -gt 0 }
    Write-Check $What $ok ("need {0} -> found {1}" -f ($Needle -join ','), ($found -join ','))
}

# Compares the produced CSV rows against a set of expected field values.
# $Expected is an array of hashtables, each shaped like:
#   @{ Id = 4102; Limit = '...'; Code = 'A'; Gekauft = '5102,00'; Free = '...' }
# Only the fields actually present in a hashtable are asserted, so the SAME
# assert can check full rows (decision-removed-refetch) and partial ones
# (free-line-from-balance only pins 'freie Linie').
#
# Matching is by the CSV account-id column 'Kto-Nr.'. Values are compared as
# strings AFTER trimming: Import-Csv can leave trailing whitespace/CRLF on the
# last column, and the exporter's decimal amounts arrive with leading spaces
# depending on how they were serialized.
function Assert-CsvRowsMatch {
    param([object[]]$Actual, [object[]]$Expected)
    foreach ($want in $Expected) {
        $line = $Actual | Where-Object { $_.'Kto-Nr.' -eq ([string]$want.Id) }
        if ($null -eq $line) {
            # Whole-row absence is a distinct, stronger failure than a field
            # mismatch, so report it independently (the exporter may not have
            # written the row at all - e.g. account rejected/empty).
            Write-Check ("CSV row {0} present" -f $want.Id) $false 'row missing'
            continue
        }
        if ($want.ContainsKey('Limit'))    { $a = $line.Limit.TrimEnd("`r").TrimEnd("`n").Trim(); Write-Check ("CSV row {0} Limit" -f $want.Id) ($a -eq [string]$want.Limit) ("expected {0} got {1}" -f $want.Limit, $a) }
        if ($want.ContainsKey('Code'))     { $a = $line.LimitKennz.Trim(); Write-Check ("CSV row {0} Code" -f $want.Id) ($a -eq [string]$want.Code) ("expected {0} got {1}" -f $want.Code, $a) }
        if ($want.ContainsKey('Gekauft'))  { $a = $line.Gekauft.Trim(); Write-Check ("CSV row {0} Gekauft" -f $want.Id) ($a -eq [string]$want.Gekauft) ("expected {0} got {1}" -f $want.Gekauft, $a) }
        if ($want.ContainsKey('Free'))     { $a = $line.'freie Linie'.Trim(); Write-Check ("CSV row {0} freie Linie" -f $want.Id) ($a -eq [string]$want.Free) ("expected {0} got {1}" -f $want.Free, $a) }
    }
}