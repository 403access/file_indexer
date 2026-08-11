# =============================================================================
# TestHarness/MockLifecycle.ps1 - Mock-CrefoApi.ps1 process lifecycle.
# Dot-sourced via TestHarness.ps1 into the runner's scope.
# =============================================================================

# Gets a free TCP port for a mock server.
function Get-FreePort {
    $l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $l.Start()
    $port = ([System.Net.IPEndPoint]$l.LocalEndpoint).Port
    $l.Stop()
    return $port
}

# Starts Mock-CrefoApi.ps1 on a free port and waits until it is ready.
function Start-Mock {
    param(
        [string]$MockFile,
        [string]$RequestLog,
        [string]$CountFile
    )
    $actualPort = Get-FreePort
    $ready = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-ready-{0}.txt" -f ([guid]::NewGuid().ToString('N')))
    $stop = Join-Path ([System.IO.Path]::GetTempPath()) ("crefo-stop-{0}.txt" -f ([guid]::NewGuid().ToString('N')))
    Remove-Item -LiteralPath $stop -Force -ErrorAction SilentlyContinue

    $proc = Start-Process -FilePath 'pwsh' -ArgumentList @(
        '-NoProfile', '-File', $mockScript,
        '-Port', $actualPort,
        '-MockFile', $mockFile,
        '-RequestLog', $RequestLog,
        '-CountFile', $CountFile,
        '-ReadyFile', $ready,
        '-StopFile', $stop) -PassThru

    $ok = $false
    for ($i = 0; $i -lt 200; $i++) {
        if ($proc.HasExited) { break }
        if (Test-Path -LiteralPath $ready) { $ok = $true; break }
        Start-Sleep -Milliseconds 50
    }
    if (-not $ok) {
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

function Stop-Mock {
    param([object]$Mock)
    New-Item -ItemType File -Path $Mock.StopFile -Force | Out-Null
    for ($i = 0; $i -lt 100; $i++) {
        if ($Mock.Process.HasExited) { break }
        Start-Sleep -Milliseconds 50
    }
    if (-not $Mock.Process.HasExited) { Stop-Process -Id $Mock.Process.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $Mock.ReadyFile, $Mock.StopFile -Force -ErrorAction SilentlyContinue
}