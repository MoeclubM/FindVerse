#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$ComposeProjectName = "findverse",
    [int]$WebPort = 3000,
    [int]$ControlApiPort = 8080,
    [int]$QueryApiPort = 8081,
    [int]$PostgresPort = 5432,
    [int]$RedisPort = 6379,
    [int]$OpenSearchPort = 9200,
    [string]$AdminUsername = "admin",
    [string]$AdminPassword = "change-me",
    [string]$CrawlerJoinKey = "",
    [string]$CrawlerServer = "http://control-api:8080",
    [int]$CrawlerMaxJobs = 10,
    [int]$CrawlerPollIntervalSecs = 5,
    [int]$CrawlerConcurrency = 4,
    [string]$CrawlerAllowedDomains = "",
    [string]$CrawlerProxy = "",
    [switch]$WithCrawler,
    [switch]$Rebuild
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

$env:COMPOSE_PROJECT_NAME = $ComposeProjectName
$env:FINDVERSE_WEB_PORT = "$WebPort"
$env:FINDVERSE_CONTROL_API_PORT = "$ControlApiPort"
$env:FINDVERSE_QUERY_API_PORT = "$QueryApiPort"
$env:FINDVERSE_POSTGRES_PORT = "$PostgresPort"
$env:FINDVERSE_REDIS_PORT = "$RedisPort"
$env:FINDVERSE_OPENSEARCH_PORT = "$OpenSearchPort"
$env:FINDVERSE_LOCAL_ADMIN_USERNAME = $AdminUsername
$env:FINDVERSE_LOCAL_ADMIN_PASSWORD = $AdminPassword
$env:FINDVERSE_CRAWLER_JOIN_KEY = $CrawlerJoinKey
$env:FINDVERSE_CRAWLER_SERVER = $CrawlerServer
$env:FINDVERSE_CRAWLER_MAX_JOBS = "$CrawlerMaxJobs"
$env:FINDVERSE_CRAWLER_POLL_INTERVAL_SECS = "$CrawlerPollIntervalSecs"
$env:FINDVERSE_CRAWLER_CONCURRENCY = "$CrawlerConcurrency"
$env:FINDVERSE_CRAWLER_ALLOWED_DOMAINS = $CrawlerAllowedDomains
$env:FINDVERSE_CRAWLER_PROXY = $CrawlerProxy

function Wait-TcpPort {
    param(
        [string]$Name,
        [string]$Host,
        [int]$Port
    )

    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        $client = New-Object System.Net.Sockets.TcpClient
        try {
            $async = $client.BeginConnect($Host, $Port, $null, $null)
            if ($async.AsyncWaitHandle.WaitOne(2000) -and $client.Connected) {
                $client.EndConnect($async)
                return
            }
        }
        catch {
        }
        finally {
            $client.Dispose()
        }
        Start-Sleep -Seconds 2
    }

    throw "$Name did not become reachable on ${Host}:$Port"
}

function Wait-Http {
    param(
        [string]$Name,
        [string]$Uri
    )

    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        try {
            $response = Invoke-WebRequest -Uri $Uri -Method GET -TimeoutSec 5 -ErrorAction Stop
            if ($response.StatusCode -ge 200 -and $response.StatusCode -lt 500) {
                return
            }
        }
        catch {
        }
        Start-Sleep -Seconds 2
    }

    throw "$Name did not become ready at $Uri"
}

$infraArgs = @("compose", "up", "-d", "postgres", "valkey", "opensearch")
$appArgs = @("compose", "up", "-d")
if ($Rebuild) {
    $appArgs += "--build"
}
$appArgs += @("control-api", "query-api", "web")
if ($WithCrawler) {
    $crawlerArgs = @("compose", "--profile", "crawler", "up", "-d")
    if ($Rebuild) {
        $crawlerArgs += "--build"
    }
    $crawlerArgs += "crawler-worker"
}

Push-Location $repoRoot
try {
    & docker @infraArgs
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose infra up failed with exit code $LASTEXITCODE"
    }

    Wait-TcpPort -Name "PostgreSQL" -Host "127.0.0.1" -Port $PostgresPort
    Wait-TcpPort -Name "Redis" -Host "127.0.0.1" -Port $RedisPort
    Wait-Http -Name "OpenSearch" -Uri "http://127.0.0.1:$OpenSearchPort"

    & docker @appArgs
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose app up failed with exit code $LASTEXITCODE"
    }

    Wait-Http -Name "Control API" -Uri "http://127.0.0.1:$ControlApiPort/healthz"
    Wait-Http -Name "Query API" -Uri "http://127.0.0.1:$QueryApiPort/readyz"

    if ($WithCrawler) {
        & docker @crawlerArgs
        if ($LASTEXITCODE -ne 0) {
            throw "docker compose crawler up failed with exit code $LASTEXITCODE"
        }
    }

    & docker compose ps
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose ps failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "Stack is running."
Write-Host "  Web:         http://127.0.0.1:$WebPort"
Write-Host "  Control API: http://127.0.0.1:$ControlApiPort"
Write-Host "  Query API:   http://127.0.0.1:$QueryApiPort"
Write-Host "  PostgreSQL:  127.0.0.1:$PostgresPort"
Write-Host "  Redis:       127.0.0.1:$RedisPort"
Write-Host "  OpenSearch:  http://127.0.0.1:$OpenSearchPort"
if ($WithCrawler) {
    Write-Host "  Crawler:     docker compose profile 'crawler' enabled"
}
