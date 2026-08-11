# =============================================================================
# CsvFormat.psm1 - CSV formatting helpers for the Crefo export:
#   - ConvertTo-CsvField / ConvertTo-GermanyNumber escape values for the
#     ';'-separated output (German decimal format)
#   - New-CsvRowFromAccount builds one output row from an account snapshot
#   - Write-CrefoCsv writes the complete CSV atomically (temp file + move)
# =============================================================================

# Escapes a value for a ';'-separated CSV: quotes fields containing ';', a
# quote, or a newline, and doubles embedded quotes.
function ConvertTo-CsvField {
    param([string]$Value)
    if ($Value -match '[;"\r\n]') {
        return ('"{0}"' -f ($Value -replace '"', '""'))
    }
    return $Value
}

# Formats a decimal German-style (comma as decimal separator, 2 places), e.g. 0,00.
function ConvertTo-GermanyNumber {
    param([object]$Value)
    if ($null -eq $Value) { return '0,00' }
    try {
        $double = [double]$Value
        return $double.ToString('F2', [System.Globalization.CultureInfo]::InvariantCulture) -replace '\.', ','
    }
    catch {
        return '0,00'
    }
}

# Builds one CSV data row from an account's stored snapshot:
#   Kto-Nr. | Name1 | Limit | LimitKennz | Gekauft | freie Linie
# freie Linie is derived from limit minus purchased (or minus balance when
# FreeLineFromBalance is enabled), never fetched.
function New-CsvRowFromAccount {
    param(
        [hashtable]$Cfg,
        [object]$Account
    )
    $limit = [double]$Account.limit
    $purchased = [double]$Account.purchased
    $balance = [double]$Account.balance
    $freeBase = if ($Cfg['FreeLineFromBalance']) { $balance } else { $purchased }
    $freeLine = $limit - $freeBase

    $fields = @(
        (ConvertTo-CsvField ([string]$Account.id)),
        (ConvertTo-CsvField ([string]$Account.name)),
        (ConvertTo-GermanyNumber $limit),
        (ConvertTo-CsvField ([string]$Account.limitCode)),
        (ConvertTo-GermanyNumber $purchased),
        (ConvertTo-GermanyNumber $freeLine)
    )
    return ($fields -join ';')
}

# Writes a complete CSV (header + all rows) atomically via a temp file.
function Write-CrefoCsv {
    param(
        [string]$Path,
        [string[]]$Rows
    )
    $utf8WithBom = New-Object System.Text.UTF8Encoding($true)
    $header = 'Kto-Nr.;Name1;Limit;LimitKennz;Gekauft;freie Linie'
    $tmp = $Path + '.tmp'
    [System.IO.File]::WriteAllLines($tmp, @($header) + @($Rows), $utf8WithBom)
    Move-Item -LiteralPath $tmp -Destination $Path -Force
}

Export-ModuleMember -Function 'ConvertTo-CsvField', 'ConvertTo-GermanyNumber', 'New-CsvRowFromAccount', 'Write-CrefoCsv'