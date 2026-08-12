# =============================================================================
# TestHarness/Fixtures.ps1 - TestData/ fixture loading + per-mock scenario
# composition. Dot-sourced via TestHarness.ps1 into the runner's scope.
#
# CONTRACT
# --------
# The mock server (Mock-CrefoApi.ps1) is a separate pwsh process that answers
# on HTTP. It needs a complete, self-contained JSON snapshot of the "world":
# which accounts exist, which limit decisions/desires exist for them, and what
# /risk payload each account serves. Nothing is shared in-memory across
# processes, so everything the mock needs is (a) read from the shared TestData/
# fixtures and (b) serialized out to a temp JSON file, which Start-Mock hands
# to the mock via -MockFile.
#
# The mapping between scenario ids and fixture entries:
#   - accounts.json   dream_id#  (keyed by account id)
#   - decisions.json  debtorNumber (the SAME value as the account id for the
#                     "decisions belong to account X" relationship)
#   - desires.json    debtorNumber (same relationship, held-open desires)
#   - risks.json      stringly-typed account-id keys -> /risk payload
#
# Select-TestWealth / New-MockScenario turn a phase's account/decision/desire
# id lists into exactly that snapshot.
# =============================================================================

# Loads one of the TestData/ JSON fixtures into a hashtable/array.
# The array-wrapping @() keeps single-element files array-shaped so callers
# can always iterate without guessing about singletons.
function Get-TestFixture {
    param([string]$Name)
    $path = Join-Path $script:TestDataDir $Name
    # ConvertFrom-Json: array top-level -> object[]; single object -> psobject.
    # The @(...) wrapper normalizes both to an array.
    return @(Get-Content -LiteralPath $path -Raw | ConvertFrom-Json)
}

# Module-level fixture tables, computed once at dot-source time (TestData rarely
# changes mid-run and reloading per phase would be needless disk IO).
$TestAccounts = Get-TestFixture 'accounts.json'
$TestDecisions = Get-TestFixture 'decisions.json'
$TestDesires = Get-TestFixture 'desires.json'
# risks.json is an OBJECT (account-id -> risk payload), so it arrives as a
# psobject. We re-key it into a hashtable whose KEYS ARE STRINGS: /risk requests
# arrive with the debitor id as a string, and a hashtable keyed by string avoids
# the all-IDs-are-ints coercion trap (eventual-string -eq) for single-digit vs
# wide ids.
$TestRisks = @{}
foreach ($entry in (Get-TestFixture 'risks.json').PSObject.Properties) {
    $TestRisks[[string]$entry.Name] = $entry.Value
}

# Picks the accounts/decisions/desires subsets for a mock from the fixtures.
# - AccountIds  : which accounts (and therefore which /risk and /creditrequests
#                 rows) the mock serves.
# - DecisionIds : which decision records exist (feed the "last decision still
#                 valid" lookups; empty for fresh-sync scenarios).
# - DesireIds   : which held-open credit desires exist (used by open-limit /
#                 free-line flows).
# Risks are taken wholesale (the mock keyed lookup filters by account at
# request time), so this stays O(3N + 1) over the fixtures, not O(N) nested.
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
# Returns the path to the temp file (caller owns cleanup via Remove-Item).
#
# Why a temp FILE and not a -MockScenario string argument: the mock is another
# pwsh process and command-line argument boundaries (spaces, quotes) plus OS
# size limits make passing a big JSON tree through the argument list fragile.
# A file sidesteps all of that and lets the mock re-read large payloads safely.
#
# -Depth 20 on ConvertTo-Json: default depth (2) would truncate the nested
# arrays of accounts/decisions/desires objects, silently producing a mock
# whose account list is empty.
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
        Faults    = $Faults   # e.g. @{ Risk401OnceIds = @(1014, 2021) }
    }
    # Copy only the risks for the accounts in this scenario into the snapshot;
    # referencing the whole shared $TestRisks would leak unrelated accounts
    # into a scenario that is meant to have NONE available.
    foreach ($id in @($Wealth.Accounts).id) {
        $risk = $Wealth.Risks[[string]$id]
        if ($null -ne $risk) { $scenario.Risk[[string]$id] = $risk }
    }
    $path = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-mock-{0}.json" -f ([guid]::NewGuid().ToString('N')))
    Set-Content -LiteralPath $path -Value ($scenario | ConvertTo-Json -Depth 20) -Encoding UTF8
    return $path
}