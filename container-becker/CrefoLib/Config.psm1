# =============================================================================
# Config.psm1 - loads the config.psd1 file, fills in sane defaults, overlays
# environment variables, resolves directory paths and validates credentials.
# Kept in its own module so Start-CrefoExport.ps1 stays a thin orchestrator.
# =============================================================================

# Returns $Default when $Value is null/empty, otherwise $Value.
function Get-Default {
    param($Value, $Default)
    if ($null -eq $Value -or [string]::IsNullOrWhiteSpace([string]$Value)) { return $Default }
    return $Value
}

# Loads the config file and returns a normalized [hashtable] ready for use.
# Throws when the file is missing or required credentials are not provided.
function Import-CrefoConfig {
    [CmdletBinding()]
    param([string]$ConfigPath)   # path to config.psd1
    if (-not (Test-Path -LiteralPath $ConfigPath)) {
        throw ("Configuration not found: '{0}'. Copy 'config.example.psd1' to 'config.psd1' and fill in your credentials." -f $ConfigPath)
    }
    $cfg = Import-PowerShellDataFile -LiteralPath $ConfigPath
    $cfgRoot = Split-Path -Parent (Resolve-Path $ConfigPath)

    # Fill in sane defaults for keys the config file may omit.
    $cfg['PageSize'] = [int](Get-Default $cfg['PageSize'] 50)
    $cfg['MaxRetries'] = [int](Get-Default $cfg['MaxRetries'] 5)
    $cfg['RequestDelayMs'] = [int](Get-Default $cfg['RequestDelayMs'] 200)
    $cfg['LogLevel'] = [string](Get-Default $cfg['LogLevel'] 'INFO')
    $cfg['RefreshAccountList'] = [bool](Get-Default $cfg['RefreshAccountList'] $true)
    $cfg['FreeLineFromBalance'] = [bool](Get-Default $cfg['FreeLineFromBalance'] $false)
    $cfg['UseLastLimitDecisions'] = [bool](Get-Default $cfg['UseLastLimitDecisions'] $true)
    $cfg['ArchiveRequests'] = [bool](Get-Default $cfg['ArchiveRequests'] $true)
    $cfg['SyncMode'] = [string](Get-Default $cfg['SyncMode'] 'Incremental')
    $cfg['MaxAgeDays'] = [int](Get-Default $cfg['MaxAgeDays'] 7)
    $cfg['OutputFileName'] = [string](Get-Default $cfg['OutputFileName'] 'crefo_limits.csv')
    $cfg['RefetchRanges'] = Get-Default $cfg['RefetchRanges'] ''

    if ($cfg['SyncMode'] -notin @('Incremental', 'RefreshAll')) {
        throw ("Invalid SyncMode '{0}'. Use 'Incremental' or 'RefreshAll'." -f $cfg['SyncMode'])
    }

    # Environment variables are optional but win over the config file (handy in CI).
    $envMap = @{
        Username     = 'CREFO_USERNAME'
        Password     = 'CREFO_PASSWORD'
        ClientId     = 'CREFO_CLIENT_ID'
        ClientSecret = 'CREFO_CLIENT_SECRET'
        BaseUrl      = 'CREFO_BASE_URL'
        ObligoNumber = 'CREFO_OBLIGO'
    }
    foreach ($key in $envMap.Keys) {
        if ([string]::IsNullOrWhiteSpace([string]$cfg[$key])) {
            $cfg[$key] = [Environment]::GetEnvironmentVariable($envMap[$key])
        }
    }

    # Resolve relative directory entries against the config file's location and
    # make sure the directories exist before we write anything into them.
    foreach ($dirKey in @('OutputDir', 'StateDir', 'LogDir', 'ArchiveDir')) {
        if ([string]::IsNullOrWhiteSpace([string]$cfg[$dirKey])) { $cfg[$dirKey] = $dirKey }
        if (-not [System.IO.Path]::IsPathRooted([string]$cfg[$dirKey])) {
            $cfg[$dirKey] = Join-Path $cfgRoot $cfg[$dirKey]
        }
        if (-not (Test-Path -LiteralPath $cfg[$dirKey])) {
            New-Item -ItemType Directory -Path $cfg[$dirKey] -Force | Out-Null
        }
    }

    # The credential fields are mandatory; fail with a helpful message otherwise.
    $required = @('BaseUrl', 'Username', 'Password', 'ClientId', 'ClientSecret')
    $missing = @($required | Where-Object { [string]::IsNullOrWhiteSpace([string]$cfg[$_]) })
    if ($missing.Count -gt 0) {
        throw ("Missing required configuration values: {0}. Provide them in '{1}' or via environment variables." -f ($missing -join ', '), $ConfigPath)
    }

    return $cfg
}

# Parses a RefetchRanges specification ("1014" or "1100-1200", comma- or
# array-separated) into normalized [pscustomobject]@{ Min; Max } ranges.
# Throws on malformed entries; a zero-entry spec returns an empty array.
function ConvertTo-CrefoRefetchRanges {
    [CmdletBinding()]
    param([object]$Value)   # string, array of strings/ints, or $null
    $ranges = New-Object System.Collections.Generic.List[object]
    $parts = @()
    if ($null -eq $Value) { return @() }
    if ($Value -is [array]) { $parts = @($Value) }
    else { $parts = @($Value -split ',') }
    foreach ($part in $parts) {
        $token = [string]$part
        if ([string]::IsNullOrWhiteSpace($token)) { continue }
        $token = $token.Trim()
        $match = [regex]::Match($token, '^(\d+)(?:-(\d+))?$')
        if (-not $match.Success) {
            throw ("Invalid RefetchRanges entry '{0}'. Use a single id like 1014 or a range like 1100-1200." -f $token)
        }
        $min = [int]$match.Groups[1].Value
        $max = if ($match.Groups[2].Success) { [int]$match.Groups[2].Value } else { $min }
        if ($max -lt $min) {
            throw ("Invalid RefetchRanges entry '{0}': range end below start." -f $token)
        }
        $ranges.Add([pscustomobject]@{ Min = $min; Max = $max })
    }
    return $ranges.ToArray()
}

Export-ModuleMember -Function 'Import-CrefoConfig', 'ConvertTo-CrefoRefetchRanges'