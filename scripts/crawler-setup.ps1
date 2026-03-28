#Requires -Version 5.1
<#
.SYNOPSIS
    FindVerse Crawler Setup
.DESCRIPTION
    Registers a crawler via join key and optionally starts the worker.
.EXAMPLE
    .\crawler-setup.ps1 -Server https://api.example.com -JoinKey my-secret-key -Start
#>
param(
    [Parameter(Mandatory)][string]$Server,
    [Parameter(Mandatory)][string]$JoinKey,
    [string]$Name = "worker-$env:COMPUTERNAME",
    [switch]$Start,
    [int]$Concurrency = 4,
    [int]$MaxJobs = 10,
    [int]$PollIntervalSecs = 5,
    [string]$AllowedDomains,
    [string]$Proxy,
    [string]$LlmBaseUrl,
    [string]$LlmApiKey,
    [string]$LlmModel,
    [double]$LlmMinScore = 0.45,
    [int]$LlmMaxBodyChars = 6000,
    [string]$BinaryPath,
    [string]$EnvFile = ".env.crawler"
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

function Start-CrawlerWorker {
    param(
        [string]$Server,
        [string]$CrawlerId,
        [string]$CrawlerKey
    )

    $workerArgs = @(
        "worker",
        "--server", $Server,
        "--crawler-id", $CrawlerId,
        "--crawler-key", $CrawlerKey,
        "--max-jobs", $MaxJobs,
        "--poll-interval-secs", $PollIntervalSecs,
        "--concurrency", $Concurrency
    )

    if ($AllowedDomains) {
        $workerArgs += @("--allowed-domains", $AllowedDomains)
    }
    if ($Proxy) {
        $workerArgs += @("--proxy", $Proxy)
    }
    if ($LlmBaseUrl -and $LlmModel) {
        $workerArgs += @(
            "--llm-base-url", $LlmBaseUrl,
            "--llm-model", $LlmModel,
            "--llm-min-score", $LlmMinScore,
            "--llm-max-body-chars", $LlmMaxBodyChars
        )
    }
    if ($LlmApiKey) {
        $workerArgs += @("--llm-api-key", $LlmApiKey)
    }

    if ($BinaryPath) {
        & $BinaryPath @workerArgs
        return
    }

    $binary = Get-Command findverse-crawler -ErrorAction SilentlyContinue
    if ($binary) {
        & $binary.Source @workerArgs
        return
    }

    $cargoPath = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargoPath) {
        & cargo run -p findverse-crawler -- @workerArgs
        return
    }

    throw "neither findverse-crawler nor cargo found in PATH"
}

# Check cached credentials
if (Test-Path $EnvFile) {
    Write-Host "Found existing credentials in $EnvFile"
    $envContent = Get-Content $EnvFile | ForEach-Object {
        if ($_ -match '^(\w+)=(.*)$') {
            [PSCustomObject]@{ Key = $Matches[1]; Value = $Matches[2] }
        }
    }
    $crawlerId = ($envContent | Where-Object Key -eq 'CRAWLER_ID').Value
    $crawlerKey = ($envContent | Where-Object Key -eq 'CRAWLER_KEY').Value

    if ($crawlerId -and $crawlerKey) {
        Write-Host "  Crawler ID: $crawlerId"
        Write-Host "  Using cached credentials. Delete $EnvFile to re-register."
        if ($Start) {
            Write-Host "Starting crawler worker..."
            Start-CrawlerWorker -Server $Server -CrawlerId $crawlerId -CrawlerKey $crawlerKey
        }
        return
    }
}

# Register
Write-Host "Registering crawler '$Name' with server $Server..."
$body = @{ join_key = $JoinKey; name = $Name } | ConvertTo-Json
$uri = "$($Server.TrimEnd('/'))/internal/crawlers/join"

try {
    $response = Invoke-RestMethod -Uri $uri -Method Post -ContentType 'application/json' -Body $body
} catch {
    Write-Error "Failed to register: $_"
    return
}

$crawlerId = $response.crawler_id
$crawlerKey = $response.crawler_key
$returnedName = $response.name

Write-Host "Registered successfully!"
Write-Host "  Crawler ID:   $crawlerId"
Write-Host "  Crawler name: $returnedName"

# Save credentials
@"
CRAWLER_ID=$crawlerId
CRAWLER_KEY=$crawlerKey
SERVER=$Server
LLM_BASE_URL=$LlmBaseUrl
LLM_MODEL=$LlmModel
"@ | Set-Content $EnvFile -Encoding UTF8

Write-Host "Credentials saved to $EnvFile"

if ($Start) {
    Write-Host "Starting crawler worker..."
    Start-CrawlerWorker -Server $Server -CrawlerId $crawlerId -CrawlerKey $crawlerKey
} else {
    Write-Host ""
    Write-Host "To start the crawler manually:"
    Write-Host "  findverse-crawler worker --server $Server --crawler-id $crawlerId --crawler-key <crawler-key> --max-jobs $MaxJobs --poll-interval-secs $PollIntervalSecs --concurrency $Concurrency"
}
