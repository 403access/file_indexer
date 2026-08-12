# =============================================================================
# Logger.psm1 - small, leveled logger for the Crefo export scripts.
# Writes timestamped lines to the console and/or a UTF-8 log file.
# Log levels are numeric so they can be compared: 10=DEBUG, 20=INFO, 30=WARN, 40=ERROR.
# =============================================================================

$script:LogFilePath  = $null    # optional log file path (empty = console only)
$script:LogLevelCode = 20       # minimum level that will be emitted
$script:LogToConsole = $true    # whether to mirror log lines to the console

function Initialize-Logger {
    [CmdletBinding()]
    param(
        [string]$LogFilePath,                                 # full path to the log file, or empty to log only to console
        [ValidateSet('DEBUG', 'INFO', 'WARN', 'ERROR')]
        [string]$Level = 'INFO',                              # minimum level to record
        [bool]$Console = $true                                # echo log lines to the console
    )
    # Map the human-readable level name to a comparable numeric code.
    $script:LogLevelCode = switch ($Level) {
        'DEBUG' { 10 }
        'INFO'  { 20 }
        'WARN'  { 30 }
        'ERROR' { 40 }
    }
    $script:LogToConsole = $Console
    if (-not [string]::IsNullOrWhiteSpace($LogFilePath)) {
        # Ensure the parent directory of the log file exists before we append.
        $dir = Split-Path -Path $LogFilePath -Parent
        if ($dir -and -not (Test-Path -LiteralPath $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
        $script:LogFilePath = $LogFilePath
    }
}

function Write-Log {
    [CmdletBinding()]
    param(
        [int]$Level,         # numeric severity of this message
        [string]$LevelName,  # display name, e.g. 'INFO'
        [string]$Message     # the message text
    )
    # Drop messages that are below the configured threshold.
    if ($Level -lt $script:LogLevelCode) { return }
    $line = '{0:yyyy-MM-dd HH:mm:ss.fff} [{1}] {2}' -f (Get-Date), $LevelName, $Message
    if ($script:LogToConsole) { Write-Host $line }
    if ($script:LogFilePath) {
        # Append (never overwrite) so a run's history is preserved.
        Add-Content -LiteralPath $script:LogFilePath -Value $line -Encoding UTF8
    }
}

# Convenience wrappers so callers do not need to remember level codes/names.

function Write-CrefoInfo {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Log -Level 20 -LevelName 'INFO' -Message $Message
}

function Write-CrefoWarn {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Log -Level 30 -LevelName 'WARN' -Message $Message
}

function Write-CrefoError {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Log -Level 40 -LevelName 'ERROR' -Message $Message
}

function Write-CrefoDebug {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Log -Level 10 -LevelName 'DEBUG' -Message $Message
}

Export-ModuleMember -Function 'Initialize-Logger', 'Write-CrefoInfo', 'Write-CrefoWarn', 'Write-CrefoError', 'Write-CrefoDebug'