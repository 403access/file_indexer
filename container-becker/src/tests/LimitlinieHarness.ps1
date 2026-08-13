# =============================================================================
# LimitlinieHarness.ps1 - support code for the Update-Limitlinie.ps1 tests
# (tests/Run-LimitlinieTests.ps1). Dot-sourced after TestHarness.ps1 into the
# runner's scope, so it can reuse Start-Mock/Stop-Mock/Read-RequestLog/
# Get-CountValue/Write-Check/Assert-Equal and the shared fixtures.
#
# Unlike Invoke-CrefoPhase (which drives Start-CrefoExport.ps1), the phase
# here drives Update-Limitlinie.ps1: the "world" is a set of document folders
# and files, and the observations are the produced archive tree + download dir
# plus the mock's request log / counters.
# =============================================================================

# Writes a mock scenario json containing one minimal account (the mock requires
# at least one Accounts entry) plus the document folders/files the Documents
# endpoints serve. -Files maps folder name -> array of document objects; every
# object needs at least 'name' and 'created'. Returns the temp file path.
function New-LimitlinieMockScenario {
    param(
        [string[]]$Folders,
        [hashtable]$Files
    )
    $scenario = @{
        Accounts        = @(@($TestAccounts)[0])
        Decisions       = @()
        Desires         = @()
        Risk            = @{}
        Faults          = @{}
        DocumentFolders = @($Folders)
        DocumentFiles   = @{}
    }
    foreach ($folder in $Folders) {
        $scenario.DocumentFiles[$folder] = @($Files[$folder])
    }
    $path = Join-Path ([System.IO.Path]::GetTempPath()) ("limitlinie-mock-{0}.json" -f ([guid]::NewGuid().ToString('N')))
    Set-Content -LiteralPath $path -Value ($scenario | ConvertTo-Json -Depth 20) -Encoding UTF8
    return $path
}

# Writes a config.psd1 for Update-Limitlinie.ps1 pointing at the mock. The
# DownloadDir/ArchiveDir live inside $RuntimeDir so a scenario's archive tree
# and download folder persist across phases. Accepts extra config overrides.
function New-LimitlinieConfig {
    param(
        [string]$RuntimeDir,
        [string]$BaseUrl,
        [hashtable]$Overrides = @{}
    )
    $archiveDir = Join-Path $RuntimeDir 'archive'
    $downloadDir = Join-Path $RuntimeDir 'download'
    foreach ($d in @($archiveDir, $downloadDir)) {
        if (-not (Test-Path -LiteralPath $d)) { New-Item -ItemType Directory -Path $d -Force | Out-Null }
    }
    $defaults = [ordered]@{
        BaseUrl      = $BaseUrl
        Username     = 'tester'
        Password     = 'tester'
        ClientId     = 'testclient'
        ClientSecret = 'testsecret'
        Directory    = 'Tagesabrechnungen'
        FileSuffix   = '_Limitlinie.csv'
        DownloadDir  = $downloadDir
        ArchiveDir   = $archiveDir
    }
    foreach ($k in $Overrides.Keys) { $defaults[$k] = $Overrides[$k] }
    $path = Join-Path $RuntimeDir ('config-limitlinie-{0}.psd1' -f ([guid]::NewGuid().ToString('N')))

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('@{')
    foreach ($k in $defaults.Keys) {
        $v = $defaults[$k]
        if ($v -is [bool]) { $serialized = if ($v) { '$true' } else { '$false' } }
        elseif ($v -is [int]) { $serialized = [string]$v }
        elseif ($v -is [psobject] -and $v -is [string]) { $serialized = "'{0}'" -f ([string]$v -replace "'", "''") }
        elseif ($v -is [string]) { $serialized = "'{0}'" -f ([string]$v -replace "'", "''") }
        elseif ($null -eq $v) { $serialized = '' }
        elseif ($v -is [System.Array]) {
            $inner = @($v | ForEach-Object { "'{0}'" -f ([string]$_ -replace "'", "''") }) -join ', '
            $serialized = "@($inner)"
        }
        else { $serialized = "'{0}'" -f ([string]$v -replace "'", "''") }
        if ([string]::IsNullOrWhiteSpace($serialized)) {
            $lines.Add(("    {0} = ''" -f $k))
        }
        else {
            $lines.Add(("    {0} = {1}" -f $k, $serialized))
        }
    }
    $lines.Add('}')
    Set-Content -LiteralPath $path -Value ($lines -join "`n") -Encoding UTF8
    return $path
}

# Runs Update-Limitlinie.ps1 as a child pwsh process against the given config.
# Returns { ExitCode, OutFile, ErrFile, Output } so a phase can assert on the
# exit code and on the console output just like a real run.
function Invoke-LimitlinieRun {
    param(
        [string]$ConfigPath,
        [string]$ScriptPath
    )
    $logFile = Join-Path ([System.IO.Path]::GetTempPath()) ("limitlinie-run-{0}-out.txt" -f ([guid]::NewGuid().ToString('N')))
    $errFile = Join-Path ([System.IO.Path]::GetTempPath()) ("limitlinie-run-{0}-err.txt" -f ([guid]::NewGuid().ToString('N')))
    $p = Start-Process -FilePath 'pwsh' -ArgumentList @('-NoProfile', '-File', $ScriptPath, '-ConfigPath', $ConfigPath) `
        -Wait -PassThru -RedirectStandardOutput $logFile -RedirectStandardError $errFile
    $output = if (Test-Path -LiteralPath $logFile) { Get-Content -LiteralPath $logFile -Raw } else { '' }
    $errText = if (Test-Path -LiteralPath $errFile) { Get-Content -LiteralPath $errFile -Raw } else { '' }
    return [pscustomobject]@{
        ExitCode = $p.ExitCode
        OutFile  = $logFile
        ErrFile  = $errFile
        Output   = "$output$errText"
    }
}