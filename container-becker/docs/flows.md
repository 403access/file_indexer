# Flows

## Overall run

```mermaid
sequenceDiagram
    participant Start as Start-CrefoExport.ps1
    participant Log as Logger/Archive
    participant DB as Database (crefo.db)
    participant St as State (crefo_state.json)
    participant API as Crefo API

    Start->>Log: init logger + archive
    Start->>DB: Initialize-CrefoDatabase (schema, WAL)
    Start->>API: Get-CrefoAccessToken (cached or fresh)
    Start->>St: Get-CrefoState
    Start->>DB: Import-CrefoDatabaseFromState (one-time seed)
    Start->>DB: Get-CrefoDatabaseAccountSummary (count, highest id)

    alt empty database
        Start->>API: full list fetch, page by page
    else known accounts
        Start->>API: probe list (pageSize=0/1) - total size
        API-->>Start: header.totalItems/totalPages
        opt production grew (gap > 0)
            Start->>API: fetch only the delta (StartIndex/MaxCount)
            Start->>St: Merge-CrefoAccounts (new debtors)
        end
        opt probe failed
            Start->>API: fall back to full list fetch
        end
    end

    Start->>API: last-limit-decisions + open-limit-desires (bulk)
    loop every account
        Start->>API: GET /risk (only if refresh needed)
        Start->>DB: Save-CrefoAccount / Save-CrefoRiskSnapshot
        Start-->>St: Save-CrefoState (resumable after each account)
    end

    Start->>DB: Get-CrefoDatabaseCsvRows
    Start->>Start: Write-CrefoCsv (head + all rows, stable order)
```

## Account list discovery (probe + delta)

```mermaid
flowchart LR
    A[Get-CrefoDatabaseAccountSummary] --> B{knownCount = 0?}
    B -- yes --> C[full list fetch page by page]
    B -- no --> D[probe pageSize=0 then 1]
    D --> E{probe ok?}
    E -- no --> C
    E -- yes --> F{gap = totalItems - knownCount}
    F -- <= 0 --> G[list unchanged - keep cached, skip sync]
    F -- > 0 --> H[delta fetch StartIndex=knownCount MaxCount=gap]
    C --> I[Merge-CrefoAccounts]
    H --> I
    G --> J[set accountListFetchedAt + Save-CrefoState]
    I --> J
```

## Per-account risk processing

```mermaid
flowchart TD
    A[for each account sorted by id] --> R{"id in RefetchRanges?"}
    R -- yes --> C[GET /risk - store snapshot source='api']
    R -- no --> B{"ShouldRefresh?<br/>new / failed / decision changed<br/>open pipeline / older than MaxAgeDays"}
    B -- yes --> C[GET /risk - store snapshot source='api']
    B -- no --> D{"account has a stored limitCode?"}
    D -- yes --> E[reuse stored snapshot - zero requests]
    D -- no --> F[short-circuit zero row - no request]
    C --> G[New-CsvRowFromAccount]
    E --> G
    F --> G
    G --> H[mark account done]
    H --> I[Save account + snapshot to database]
    I --> J[Save state after every account]
    J --> A
```

## CSV rebuild (database is the source of truth)

```mermaid
flowchart TD
    A[Get-CrefoDatabaseCsvRows] --> B["latest risk snapshot per account<br/>via MAX(id) grouped join"]
    B --> C[skip accounts with status = failed]
    C --> D[order by account id]
    D --> E{"database returned rows?"}
    E -- yes --> F[Write-CrefoCsv from database rows]
    E -- no --> G[fall back to in-memory rows]
```

## Test harness (one phase)

```mermaid
sequenceDiagram
    participant Run as Run-CrefoTests.ps1
    participant FS as temp runtime dir
    participant Mock as Mock-CrefoApi.ps1 (child)
    participant Exp as Start-CrefoExport.ps1 (child)

    Run->>Run: Select-TestWealth (fixtures) + New-MockScenario (json)
    Run->>Run: get free TCP port
    Run->>Mock: Start-Process pwsh -File Mock-CrefoApi.ps1
    Mock->>FS: ready file (listener up, port)
    Run->>Run: wait until ready
    Run->>FS: New-RunConfig -BaseUrl = mock port (config.psd1)
    Run->>Exp: Start-Process pwsh Start-CrefoExport.ps1 -ConfigPath
    loop every API call
        Exp->>Mock: token / list / probe / decisions / desires / risk
        Mock->>FS: append request log line + save counters
    end
    Exp->>FS: write CSV, state, crefo.db, log
    Exp-->>Run: exit code
    Run->>Mock: touch stop file, wait for exit
    Run->>FS: read request log, counters, CSV, log
    Run->>Run: assertions (RiskIds / CsvRows / ExitCode / LogContains)
```

## Scenario / phase execution

```mermaid
flowchart TD
    Start[selected scenarios] --> S{next scenario}
    S -- yes --> R[create runtime dir]
    R --> P{next phase}
    P -- yes --> W[Select-TestWealth from fixtures]
    W --> M[New-MockScenario: accounts + decisions + desires + risks + faults]
    M --> MS[Start-Mock on free port]
    MS --> C[New-RunConfig -BaseUrl mock port]
    C --> E[run exporter as child pwsh]
    E --> ST[Stop-Mock]
    ST --> A[assert ExitCode / RiskIds / CsvRowCount / CsvRows / LogContains]
    A --> Res{phase passed?}
    Res -- no --> FP[scenario FAIL]
    Res -- yes --> NextP{more phases?}
    NextP -- yes --> P
    NextP -- no --> OK[scenario PASS - next scenario]
    FP --> NextS{more scenarios?}
    OK --> NextS
    NextS -- yes --> S
    NextS -- no --> Sum[summary 15 scenarios - exit 0 or 1]
```

## Mock server routing & fault injection

```mermaid
flowchart LR
    A[HttpListener] --> B{path?}
    B -- /connect/token --> T[token response, counters.token++]
    B -- /DebitorAccounts/list-debitor --> L{pageSize <= 1?}
    L -- yes --> LP[probe: header.totalItems only<br/>fault: Probe500]
    L -- no --> LF[full page: slice accounts by page/pagesize]
    B -- /last-limit-decisions --> D[send decisions array<br/>fault: Decisions500]
    B -- /open-limit-desires --> Q[send desires array<br/>fault: Desires500]
    B -- /DebitorAccounts/{id}/risk --> R{fault?}
    R -- "id in Risk500Ids" --> R500[HTTP 500]
    R -- "Risk401OnceIds first hit" --> R401[HTTP 401 once, then succeed]
    R -- "Risk500OnceId first hit" --> R500O[HTTP 500 once, then succeed]
    R -- none --> RX[risk payload from scenario or derived from id]
    T --> Log[append request log + save counters]
    LP --> Log
    LF --> Log
    D --> Log
    Q --> Log
    R500 --> Log
    R401 --> Log
    R500O --> Log
    RX --> Log
```
