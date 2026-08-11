# =============================================================================
# CrefoApi/Accounts.ps1 - debitor account list discovery.
# Get-CrefoDebtorListStats probes the production list size with the smallest
# possible request (pageSize=0, falling back to 1). Get-CrefoAccounts walks the
# pages and collects (id, name); -StartIndex/-MaxCount restrict the walk to the
# trailing slice for delta sync.
# =============================================================================

function Get-CrefoDebtorListStats {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'list-probe' # archive folder label for the probe
    )
    # Smallest possible probe of GET /api/v1/DebitorAccounts/list-debitor to
    # learn the production list size: header.totalItems/totalPages are returned
    # regardless of pageSize, so pageSize=0 (falling back to 1 where 0 is
    # rejected) gives the exact total in the smallest payload. The caller uses
    # this to detect a gap against the database and fetch only the difference.
    $probe = $null
    $usedPageSize = 0
    foreach ($tryPageSize in @(0, 1)) {
        try {
            $probe = Invoke-CrefoApi -Config $Config -Method GET `
                -Path '/api/v1/DebitorAccounts/list-debitor' `
                -AccessToken $AccessToken `
                -Query @{ page = 1; pagesize = $tryPageSize } `
                -AuthRefresher $AuthRefresher `
                -ArchiveName $ArchiveName
            $usedPageSize = $tryPageSize
            break
        }
        catch {
            # pageSize=0 may be rejected by some API versions; retry with 1.
            if ($tryPageSize -eq 0) { continue }
            throw
        }
    }
    $totalItems = $null
    $totalPages = $null
    if ($null -ne $probe -and $null -ne $probe.header) {
        if ($probe.header.PSObject.Properties.Name -contains 'totalItems') {
            $totalItems = [int]$probe.header.totalItems
        }
        if ($probe.header.PSObject.Properties.Name -contains 'totalPages') {
            $raw = $probe.header.totalPages
            if ($null -ne $raw -and $raw -ne '') {
                $totalPages = [long]$raw
                if ($totalPages -lt 0 -or $totalPages -gt [long]::MaxValue) {
                    $totalPages = $null
                }
            }
        }
    }
    if ($null -eq $totalItems) {
        throw 'Account list probe response did not include header.totalItems.'
    }
    $totalPagesText = if ($null -ne $totalPages) { $totalPages } else { 'n/a' }
    Write-CrefoInfo ("Account list probe (pageSize={0}): {1} total item(s) / {2} page(s)." -f $usedPageSize, $totalItems, $totalPagesText)
    return [pscustomobject]@{
        TotalItems = $totalItems
        TotalPages = $totalPages
    }
}

function Get-CrefoAccounts {
    [CmdletBinding()]
    param(
        [hashtable]$Config,                 # configuration
        [string]$AccessToken,               # Bearer token
        [int]$PageSize = 50,                # items per page (API default: 50)
        [scriptblock]$AuthRefresher,        # passed through to enable 401 recovery
        [string]$ArchiveName = 'list-debitor',   # archive folder label for these pages
        [int]$StartIndex = 0,               # first account index to return (skip the leading entries)
        [int]$MaxCount = 0                  # max accounts to return (0 = no limit)
    )
    # Walk the pages of GET /api/v1/DebitorAccounts/list-debitor and collect
    # the (id, name) pairs. Pagination ends when we pass totalPages or receive
    # a short page. A safety cap guards against an API that never returns a
    # short page nor a usable totalPages (it would otherwise loop forever).
    #
    # With StartIndex/MaxCount this walks only the trailing slice of the list
    # (used for delta sync: fetch just the accounts after the ones we know).
    # Pages are 1-based with PageSize entries; the first page that can carry the
    # requested offset is found by integer division, and the leading entries on
    # that page that fall before StartIndex are skipped.
    $all = New-Object System.Collections.Generic.List[object]
    $skipPages = if ($StartIndex -gt 0) { [int][math]::Floor($StartIndex / $PageSize) } else { 0 }
    $skipInPage = if ($StartIndex -gt 0) { $StartIndex % $PageSize } else { 0 }
    $startPage = [math]::Max(1, $skipPages + 1)
    $page = $startPage
    $totalPages = $null
    $pageCap = 10000
    $emptyPages = 0

    while ($page -le $pageCap) {
        if ($totalPages -eq $null) {
            Write-CrefoInfo ("Fetching account list: page {0}..." -f $page)
        }
        else {
            Write-CrefoInfo ("Fetching account list: page {0}/{1}..." -f $page, $totalPages)
        }
        $response = Invoke-CrefoApi -Config $Config -Method GET `
            -Path '/api/v1/DebitorAccounts/list-debitor' `
            -AccessToken $AccessToken `
            -Query @{ page = $page; pagesize = $PageSize } `
            -AuthRefresher $AuthRefresher `
            -ArchiveName $ArchiveName

        $rawItems = @($response.items)
        $items = $rawItems
        # Drop the leading entries on the page that contains StartIndex.
        if ($page -eq $startPage -and $skipInPage -gt 0) {
            if ($items.Count -le $skipInPage) { $items = @() }
            else { $items = @($items | Select-Object -Skip $skipInPage) }
        }
        foreach ($item in $items) {
            if ($null -ne $item.id) {
                $all.Add([PSCustomObject]@{
                    id   = [int]$item.id
                    name = [string]$item.name
                })
            }
            if ($MaxCount -gt 0 -and $all.Count -ge $MaxCount) { break }
        }
        Write-CrefoInfo ("Account list: page {0} returned {1} account(s), {2} total so far." -f $page, $rawItems.Count, $all.Count)

        # Stop once the delta target is reached (no need to walk the rest).
        if ($MaxCount -gt 0 -and $all.Count -ge $MaxCount) { break }

        # Remember totalPages from the very first response (it is stable).
        if ($null -eq $totalPages -and $null -ne $response.header) {
            $raw = $response.header.totalPages
            if ($null -ne $raw -and $raw -ne '') {
                $totalPages = [long]$raw
                if ($totalPages -lt 0) { $totalPages = $null }
            }
        }
        $page++
        if ($null -ne $totalPages -and $page -gt $totalPages) { break }
        if (@($rawItems).Count -lt $PageSize) { break }

        # Guard against degenerate APIs: several consecutive empty pages mean the
        # pagination is not making progress, stop instead of looping forever.
        if (@($rawItems).Count -eq 0) {
            $emptyPages++
            if ($emptyPages -ge 3) {
                Write-CrefoWarn 'Account list pagination is not making progress (3 empty pages); stopping.'
                break
            }
        }
        else {
            $emptyPages = 0
        }
        if ($page -gt $pageCap) {
            Write-CrefoWarn ("Account list pagination hit the safety cap of {0} pages; stopping." -f $pageCap)
            break
        }
    }
    # ToArray() avoids a PowerShell quirk where wrapping a List[object] created
    # via New-Object in @() throws "Argument types do not match".
    return $all.ToArray()
}