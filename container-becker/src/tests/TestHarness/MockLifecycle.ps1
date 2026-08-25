# =============================================================================
# TestHarness/MockLifecycle.ps1 - Mock-CrefoApi.ps1 process lifecycle.
# Dot-sourced via TestHarness.ps1 into the runner's scope.
#
# START/STOP HANDSHAKE
# --------------------
# The mock and the runner are separate processes with no shared memory, so we
# coordinate using marker FILES in the temp dir:
#   - ReadyFile : written by the mock once it is listening; the runner polls
#                 for it (bounded) instead of guessing a sleep duration, which
#                 would either race (port not bound yet) or waste seconds.
#   - StopFile  : written by the runner to signal a graceful shutdown; the mock
#                 polls for it between requests. Graceful beats Kill because it
#                 lets the mock flush its count + request-log files before it
#                 exits (those files are the observation points of a phase).
#
# Every mock also needs its OWN free TCP port. We ask the OS for one by binding
# to ephemeral port 0; the same trick is reused elsewhere when a fresh port is
# required. There is a small TOCTOU window between returning the port and the
# mock binding it, but it is harmless here (nothing else listens on it).
# =============================================================================

# Gets a free TCP port for a mock server. Bind-to-0 asks the OS for the next
# unused ephemeral port, then we release it and hand the number to the mock.
function Get-FreePort {
    $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $l.Start()
    $port = ([System.Net.IPEndPoint]$l.LocalEndpoint).Port
    $l.Stop()
    return $port
}

# Starts Mock-CrefoApi.ps1 on a free port and waits until it is ready.
# Returns a Mock handle object used by Invoke-CrefoPhase (and Stop-Mock):
#   Port / Base - what the exporter's config must point at (Base = http://...)
#   Process     - handle for HasExited checks + force-kill fallback
#   ReadyFile   - handshake marker the mock writes once it is listening
#   StopFile    - the file Stop-Mock touches to request a graceful shutdown
function Start-Mock {
    param(
        [string]$MockFile,    # -MockFile from New-MockScenario
        [string]$RequestLog,  # jsonl path where the mock logs every request
        [string]$CountFile    # json path where the mock keeps hit counters
    )
    $actualPort = Get-FreePort
    $ready = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-ready-{0}.txt" -f ([guid]::NewGuid().ToString('N')))
    $stop = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-stop-{0}.txt" -f ([guid]::NewGuid().ToString('N')))
    # A leftover stop-file from a previous, forcefully-killed run would tell a
    # fresh mock to shut down immediately, so clear it before spawning.
    Remove-Item -LiteralPath $stop -Force -ErrorAction SilentlyContinue

    # Start the mock as a SEPARATE pwsh process: the exporter is itself started
    # as a child later, and running the mock inside the runner's process would
    # block/contaminate the shared script scope.
    $proc = Start-Process -FilePath 'pwsh' -ArgumentList @(
        '-NoProfile', '-File', $mockScript,
        '-Port', $actualPort,
        '-MockFile', $mockFile,
        '-RequestLog', $RequestLog,
        '-CountFile', $CountFile,
        '-ReadyFile', $ready,
        '-StopFile', $stop) -PassThru

    # Wait up to ~10s (200 * 50ms) for the ready marker; abort between polls if
    # the process has already exited (e.g. bad mock file -> parse error).
    $ok = $false
    for ($i = 0; $i -lt 200; $i++) {
        if ($proc.HasExited) { break }
        if (Test-Path -LiteralPath $ready) { $ok = $true; break }
        Start-Sleep -Milliseconds 50
    }
    if (-not $ok) {
        # Fail loudly rather than letting the caller start an exporter against
        # a dead port and produce a confusing "connection failed" assertion.
        throw "Mock server did not become ready (port $actualPort)."
    }
    return [pscustomobject]@{
        Port = $actualPort
        ReadyFile = $ready
        StopFile = $stop
        Process = $proc
        Base = "http://127.0.0.1:$actualPort"
    }
}

# Stops a mock started by Start-Mock. Prefers the graceful stop-file handshake
# and only force-kills as a timeout fallback (so count/request-log flushing is
# attempted). Always cleans up the handshake markers so the temp dir stays tidy
# and a stale ready-file can never confuse a future start.
function Stop-Mock {
    param([object]$Mock)
    New-Item -ItemType File -Path $Mock.StopFile -Force | Out-Null
    for ($i = 0; $i -lt 100; $i++) {   # ~5s grace period for a graceful exit
        if ($Mock.Process.HasExited) { break }
        Start-Sleep -Milliseconds 50
    }
    # If the mock is still alive it is wedged; nothing it can flush is going to
    # be usable, so a hard kill is the only way to unblock the next phase.
    if (-not $Mock.Process.HasExited) { Stop-Process -Id $Mock.Process.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $Mock.ReadyFile, $Mock.StopFile -Force -ErrorAction SilentlyContinue
}