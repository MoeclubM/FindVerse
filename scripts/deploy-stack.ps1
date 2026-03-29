#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$ComposeProjectName = "findverse",
    [string[]]$ComposeFile = @("docker-compose.yml"),
    [string]$EnvFile = "",
    [int]$WebPort = 3000,
    [int]$ControlApiPort = 8080,
    [int]$QueryApiPort = 8081,
    [int]$PostgresPort = 5432,
    [int]$RedisPort = 6379,
    [int]$OpenSearchPort = 9200,
    [string]$AdminUsername = "admin",
    [string]$AdminPassword = "change-me",
    [string]$CrawlerJoinKey = "change-me",
    [string]$CrawlerServer = "http://control-api:8080",
    [int]$CrawlerMaxJobs = 16,
    [int]$CrawlerPollIntervalSecs = 5,
    [int]$CrawlerConcurrency = 16,
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
$env:FINDVERSE_ENV_FILE = $EnvFile
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

function Prune-RebuildArtifacts {
    $services = @("control-api", "query-api", "web")
    if ($WithCrawler) {
        $services += "crawler-worker"
    }

    $images = $services | ForEach-Object { "$ComposeProjectName-$_" }

    Invoke-Compose stop @services 2>$null | Out-Null
    Invoke-Compose rm -f @services 2>$null | Out-Null
    & docker image rm -f @images 2>$null | Out-Null
    & docker image prune -f 2>$null | Out-Null
    & docker builder prune -f 2>$null | Out-Null
}

function Invoke-Compose {
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$Arguments
    )

    $composeArgs = @("compose")
    if (-not [string]::IsNullOrWhiteSpace($EnvFile)) {
        $composeArgs += @("--env-file", $EnvFile)
    }
    foreach ($file in $ComposeFile) {
        $composeArgs += @("-f", $file)
    }
    $composeArgs += $Arguments
    & docker @composeArgs
}

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

function Sync-CrawlerJoinKey {
    $adminSession = Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:$ControlApiPort/v1/admin/session/login" -ContentType "application/json" -Body (@{
        username = $AdminUsername
        password = $AdminPassword
    } | ConvertTo-Json -Compress) -TimeoutSec 30

    if ([string]::IsNullOrWhiteSpace($adminSession.token)) {
        throw "failed to obtain admin token while syncing crawler join key"
    }

    $response = Invoke-WebRequest -Method Put -Uri "http://127.0.0.1:$ControlApiPort/v1/admin/crawler-join-key" -Headers @{
        Authorization = "Bearer $($adminSession.token)"
    } -ContentType "application/json" -Body (@{
        join_key = $CrawlerJoinKey
    } | ConvertTo-Json -Compress) -TimeoutSec 30

    if ([int]$response.StatusCode -ne 204) {
        throw "failed to sync crawler join key via control api"
    }
}

$appArgs = @("up", "-d")
if ($Rebuild) {
    $appArgs += "--build"
}
$appArgs += @("control-api", "query-api", "web")
if ($WithCrawler) {
    $crawlerArgs = @("--profile", "crawler", "up", "-d")
    if ($Rebuild) {
        $crawlerArgs += "--build"
    }
    $crawlerArgs += "crawler-worker"
}

Push-Location $repoRoot
try {
    if ($Rebuild) {
        Prune-RebuildArtifacts
    }

    Invoke-Compose up -d postgres valkey opensearch
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose infra up failed with exit code $LASTEXITCODE"
    }

    Wait-TcpPort -Name "PostgreSQL" -Host "127.0.0.1" -Port $PostgresPort
    Wait-TcpPort -Name "Redis" -Host "127.0.0.1" -Port $RedisPort
    Wait-Http -Name "OpenSearch" -Uri "http://127.0.0.1:$OpenSearchPort/_cluster/health?wait_for_status=yellow&timeout=60s"

    Invoke-Compose @appArgs
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose app up failed with exit code $LASTEXITCODE"
    }

    Wait-Http -Name "Control API" -Uri "http://127.0.0.1:$ControlApiPort/healthz"
    Wait-Http -Name "Query API" -Uri "http://127.0.0.1:$QueryApiPort/readyz"

    if ($WithCrawler) {
        Sync-CrawlerJoinKey
        Invoke-Compose @crawlerArgs
        if ($LASTEXITCODE -ne 0) {
            throw "docker compose crawler up failed with exit code $LASTEXITCODE"
        }
    }

    Invoke-Compose ps
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose ps failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "Stack is running."
if (-not [string]::IsNullOrWhiteSpace($EnvFile)) {
    Write-Host "  Env file:    $EnvFile"
}
Write-Host "  Compose:     $($ComposeFile -join ', ')"
Write-Host "  Web:         http://127.0.0.1:$WebPort"
Write-Host "  Control API: http://127.0.0.1:$ControlApiPort"
Write-Host "  Query API:   http://127.0.0.1:$QueryApiPort"
Write-Host "  PostgreSQL:  127.0.0.1:$PostgresPort"
Write-Host "  Redis:       127.0.0.1:$RedisPort"
Write-Host "  OpenSearch:  http://127.0.0.1:$OpenSearchPort"
if ($WithCrawler) {
    Write-Host "  Crawler:     docker compose profile 'crawler' enabled"
}
