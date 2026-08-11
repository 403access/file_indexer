# =============================================================================
# TestHarness/Fixtures.ps1 - TestData/ fixture loading + per-mock scenario
# composition. Dot-sourced via TestHarness.ps1 into the runner's scope.
# =============================================================================

# Loads one of the TestData/ JSON fixtures into a hashtable/array.
function Get-TestFixture {
    param([string]$Name)
    $path = Join-Path $script:TestDataDir $Name
    return @(Get-Content -LiteralPath $path -Raw | ConvertFrom-Json)
}

$TestAccounts = Get-TestFixture 'accounts.json'
$TestDecisions = Get-TestFixture 'decisions.json'
$TestDesires = Get-TestFixture 'desires.json'
$TestRisks = @{}
foreach ($entry in (Get-TestFixture 'risks.json').PSObject.Properties) {
    $TestRisks[[string]$entry.Name] = $entry.Value
}

# Picks the accounts/decisions/desires subsets for a mock from the fixtures.
function Select-TestWealth {
    param(
        [int[]]$AccountIds,
        [int[]]$DecisionIds = @(),
        [int[]]$DesireIds = @()
    )
    return [pscustomobject]@{
        Accounts  = @($TestAccounts | Where-Object { $_.id -in $AccountIds })
        Decisions = @($TestDecisions | Where-Object { $_.debtorNumber -in $DecisionIds })
        Desires   = @($TestDesires | Where-Object { $_.debtorNumber -in $DesireIds })
        Risks     = $TestRisks
    }
}

# Writes a mock scenario json for Mock-CrefoApi.ps1 from a phase definition.
function New-MockScenario {
    param(
        [object]$Wealth,                 # Select-TestWealth result
        [string]$RiskFile = '',          # optional per-id risk overrides json path
        [hashtable]$Faults = @{}
    )
    $scenario = @{
        Accounts  = @($Wealth.Accounts)
        Decisions = @($Wealth.Decisions)
        Desires   = @($Wealth.Desires)
        Risk      = @{}
        Faults    = $Faults
    }
    foreach ($id in @($Wealth.Accounts).id) {
        $risk = $Wealth.Risks[[string]$id]
        if ($null -ne $risk) { $scenario.Risk[[string]$id] = $risk }
    }
    $path = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-mock-{0}.json" -f ([guid]::NewGuid().ToString('N')))
    Set-Content -LiteralPath $path -Value ($scenario | ConvertTo-Json -Depth 20) -Encoding UTF8
    return $path
}