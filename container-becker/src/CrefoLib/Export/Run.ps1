# =============================================================================
# CrefoLib/Export/Run.ps1 - the exporter's main processing loop and CSV rebuild.
# Dot-sourced by Start-CrefoExport.ps1 into the script scope so it can share
# $script:cfg, $script:token, $script:authRefresher, $script:forceRanges,
# $script:state and $script:statePath with the other Export feature files.
#
# Invoke-CrefoExportRun:
#   1. bulk limit-workflow endpoints (last-limit-decisions + open-limit-desires)
#   2. decisions-signature diff (detect decisions removed since the last run)
#   3. per-account loop: /risk, short-circuit zero row, or reuse stored snapshot
#   4. rebuild the complete CSV from the SQLite database
#   5. summarize the run and return the exit code (0 = success, 1 = retry)
# =============================================================================

function Invoke-CrefoExportRun {
    $allAccounts = @($script:state.accounts | Sort-Object -Property id)
    if ($allAccounts.Count -eq 0) {
        Write-CrefoInfo 'No accounts known - nothing to do.'
        return 0
    }
    $pendingCount = @($allAccounts | Where-Object { $_.status -eq 'pending' }).Count
    $doneCount = @($allAccounts | Where-Object { $_.status -eq 'done' }).Count
    $failedCount0 = @($allAccounts | Where-Object { $_.status -eq 'failed' }).Count
    Write-CrefoInfo ("Accounts in scope: {0} total ({1} pending, {2} done, {3} failed)." -f $allAccounts.Count, $pendingCount, $doneCount, $failedCount0)

    # Bulk limit-workflow endpoints. Their union defines the accounts with a live
    # limit context: only accounts in NEITHER list are safe to short-circuit to a
    # 0,00 / N row without a /risk request. If the bulk calls fail we degrade to
    # fetching /risk for every account.
    $limitDecisions = @{}
    $openDesires = @{}
    if ($script:cfg['UseLastLimitDecisions']) {
        try {
            Write-CrefoInfo 'Fetching completed limit decisions (bulk call)...'
            foreach ($decision in Get-CrefoLastLimitDecisions -Config $script:cfg -AccessToken $script:token -AuthRefresher $script:authRefresher) {
                if ($null -ne $decision -and $null -ne $decision.debtorNumber) {
                    $limitDecisions[[int]$decision.debtorNumber] = $decision
                }
            }
            Write-CrefoInfo ("Limit decisions available for {0} account(s)." -f $limitDecisions.Count)
        }
        catch {
            Write-CrefoWarn ("Could not fetch last-limit-decisions ({0}); falling back to fetching /risk for every account this run." -f $_.Exception.Message)
            $limitDecisions = @{}
            $openDesires = @{}
            $script:cfg['UseLastLimitDecisions'] = $false
        }
    }
    if ($script:cfg['UseLastLimitDecisions']) {
        try {
            Write-CrefoInfo 'Fetching open limit desires (bulk call)...'
            foreach ($desire in Get-CrefoOpenLimitDesires -Config $script:cfg -AccessToken $script:token -AuthRefresher $script:authRefresher) {
                if ($null -ne $desire -and $null -ne $desire.debtorNumber) {
                    $openDesires[[int]$desire.debtorNumber] = $desire
                }
            }
            Write-CrefoInfo ("Open limit desires available for {0} account(s)." -f $openDesires.Count)
        }
        catch {
            Write-CrefoWarn ("Could not fetch open-limit-desires ({0}); proceeding without the open-desire refinements." -f $_.Exception.Message)
            $openDesires = @{}
        }
    }

    # Detect whether the bulk decisions set changed since the last run. We keep
    # the previous set of debtor ids that had completed decisions; an account that
    # was previously in that set but is missing now is a "decision removed" case
    # and must be re-fetched even when the rest of the set is otherwise stable.
    $currentDecisionIds = [System.Collections.Generic.HashSet[int]]::new()
    foreach ($id in $limitDecisions.Keys) {
        [void]$currentDecisionIds.Add([int]$id)
    }
    $previousDecisionIds = [System.Collections.Generic.HashSet[int]]::new()
    if ($null -ne $script:state.decisionsSignature) {
        foreach ($token in ($script:state.decisionsSignature -split '\|')) {
            if ($token -ne '') { [void]$previousDecisionIds.Add([int]$token) }
        }
    }
    $newSignature = ($currentDecisionIds | Sort-Object -Unique) -join '|'
    $script:state | Add-Member -NotePropertyName decisionsSignature -NotePropertyValue $newSignature -Force
    Write-CrefoDebug ("Decisions: previous={0}, current={1}" -f ($previousDecisionIds -join ','), ($currentDecisionIds -join ','))

    $csvPath = Join-Path $script:cfg['OutputDir'] $script:cfg['OutputFileName']
    $rows = New-Object System.Collections.Generic.List[string]
    $refreshed = 0
    $reused = 0
    $shortCircuited = 0
    $failedCount = 0
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $stateSaveCounter = 0
    $stateSaveInterval = 100

    foreach ($account in $allAccounts) {
        $id = [int]$account.id
        $snapshotSource = $null   # 'api' | 'short-circuit' when a new snapshot is written
        try {
            $hasDecision = $limitDecisions.ContainsKey($id)
            $decision = if ($hasDecision) { $limitDecisions[$id] } else { $null }
            $inOpenDesires = $openDesires.ContainsKey($id)

            $accountDecisionRemoved = ($previousDecisionIds.Contains($id) -and -not $currentDecisionIds.Contains($id))
            $refreshDecision = Get-RefreshDecision -Cfg $script:cfg -Account $account -HasDecision $hasDecision -Decision $decision -InOpenDesires $inOpenDesires -DecisionsChanged $accountDecisionRemoved -ForceRanges $script:forceRanges
            if ($refreshDecision.ShouldRefresh) {
                Write-CrefoInfo ("Fetching risk data for debitor {0} ({1}) [{2}]..." -f $id, $account.name, $refreshDecision.Reason)
                $risk = Get-CrefoDebtorRisk -Config $script:cfg -DebtorId $id -AccessToken $script:token -AuthRefresher $script:authRefresher
                $account = Set-AccountSnapshot -Account $account -Risk $risk
                $snapshotSource = 'api'
                $refreshed++
            }
            elseif ($null -eq $account.limitCode) {
                # No decision, not in the open pipeline and no stored snapshot yet:
                # write the explicit N / zero row without a risk request.
                Write-CrefoInfo ("Debitor {0} ({1}) - no limit context, writing zero row [{2}]." -f $id, $account.name, $refreshDecision.Reason)
                $account = Set-AccountSnapshot -Account $account -Risk $null
                $snapshotSource = 'short-circuit'
                $shortCircuited++
            }
            else {
                $reused++
                Write-CrefoDebug ("Reusing snapshot for debitor {0} ({1}) [{2}]: limit {3}, code {4}." -f $id, $account.name, $refreshDecision.Reason, (ConvertTo-GermanyNumber $account.limit), $account.limitCode)
            }

            $rows.Add((New-CsvRowFromAccount -Cfg $script:cfg -Account $account))
            $account.status = 'done'
            $account.error = $null
        }
        catch {
            # Keep the failed account in state as 'failed' so it is retried on the
            # next run. Failed accounts keep their last snapshot but produce no row
            # in the rebuilt CSV until a later run succeeds for them.
            $failedCount++
            $account.status = 'failed'
            $account.error = $_.Exception.Message
            Write-CrefoError ("Debitor {0} failed: {1}" -f $id, $_.Exception.Message)
        }
        $account.updatedAt = (Get-Date).ToUniversalTime().ToString('o')

        # Persist to the SQLite database (canonical store) alongside the JSON state.
        try {
            Save-CrefoAccount -Account $account
            if ($null -ne $snapshotSource) {
                Save-CrefoRiskSnapshot -AccountId $id -Risk ([pscustomobject]@{
                    limit     = [double]$account.limit
                    purchased = [double]$account.purchased
                    balance   = [double]$account.balance
                    limitCode = [string]$account.limitCode
                    fetchedAt = ([string]$account.fetchedAt)
                }) -Source $snapshotSource -RiskFetched ($snapshotSource -eq 'api')
            }
        }
        catch {
            Write-CrefoWarn ("Database write failed for debitor {0}: {1}" -f $id, $_.Exception.Message)
        }

        # Persist progress periodically (batched) so a Ctrl+C or crashed run is
        # resumable without re-fetching the whole book, but without the overhead
        # of serializing the full state JSON to disk on every single account.
        $stateSaveCounter++
        if ($stateSaveCounter -ge $stateSaveInterval) {
            Save-CrefoState -Path $script:statePath -State $script:state
            $stateSaveCounter = 0
        }

        # Be polite to the API between requests (configurable) — only when we
        # actually made a request. Reused snapshots cost zero network I/O, so
        # sleeping here just burns time for no benefit.
        if ($script:cfg['RequestDelayMs'] -gt 0 -and $null -ne $snapshotSource) {
            Start-Sleep -Milliseconds $script:cfg['RequestDelayMs']
        }
    }
    $stopwatch.Stop()

    # Final state flush (in case the last batch wasn't full).
    Save-CrefoState -Path $script:statePath -State $script:state

    # Rebuild the complete CSV from the database (the canonical source). Falls back
    # to the in-memory rows if the DB read unexpectedly fails.
    try {
        $dbRows = Get-CrefoDatabaseCsvRows
        $dbRowLines = @($dbRows | ForEach-Object { New-CsvRowFromAccount -Cfg $script:cfg -Account $_ })
        if ($dbRowLines.Count -gt 0) {
            Write-CrefoCsv -Path $csvPath -Rows $dbRowLines
            Write-CrefoInfo ("CSV rebuilt from database ({0} rows)." -f $dbRowLines.Count)
        }
        else {
            Write-CrefoWarn 'Database returned no CSV rows (empty store); falling back to in-memory rows.'
            Write-CrefoCsv -Path $csvPath -Rows $rows.ToArray()
        }
    }
    catch {
        Write-CrefoWarn ("Could not rebuild CSV from database ({0}); falling back to in-memory rows." -f $_.Exception.Message)
        Write-CrefoCsv -Path $csvPath -Rows $rows.ToArray()
    }

    Write-CrefoInfo ("Run finished: total={0} refreshed={1} reused={2} short-circuited={3} failed={4} elapsed={5:N1}s" -f $allAccounts.Count, $refreshed, $reused, $shortCircuited, $failedCount, $stopwatch.Elapsed.TotalSeconds)
    if ($failedCount -gt 0) {
        Write-CrefoWarn 'Some accounts failed and are persisted in state. Re-run this script later to retry them.'
        return 1
    }
    return 0
}