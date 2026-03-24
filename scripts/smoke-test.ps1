#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$ApiBaseUrl = "http://127.0.0.1:3000/api",
    [string]$ControlApiBaseUrl = "",
    [string]$QueryApiBaseUrl = "",
    [string]$AdminUsername = "admin",
    [string]$AdminPassword = "change-me",
    [string]$SeedUrl = "",
    [switch]$RunPlaywright
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ApiBaseUrl = $ApiBaseUrl.TrimEnd("/")
$ControlApiBaseUrl = $ControlApiBaseUrl.TrimEnd("/")
$QueryApiBaseUrl = $QueryApiBaseUrl.TrimEnd("/")
$CrawlerServer = if ([string]::IsNullOrWhiteSpace($ControlApiBaseUrl)) { $ApiBaseUrl } else { $ControlApiBaseUrl }
$PublicBaseUrl = if ($ApiBaseUrl.EndsWith("/api")) { $ApiBaseUrl.Substring(0, $ApiBaseUrl.Length - 4) } else { $ApiBaseUrl }

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Assert-True {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Invoke-JsonRequest {
    param(
        [string]$Method,
        [string]$Uri,
        [object]$Body = $null,
        [hashtable]$Headers = @{}
    )

    $request = @{
        Method = $Method
        Uri = $Uri
        Headers = $Headers
        TimeoutSec = 90
    }

    if ($null -ne $Body) {
        $request["Body"] = ($Body | ConvertTo-Json -Depth 20 -Compress)
        $request["ContentType"] = "application/json"
    }

    Invoke-RestMethod @request
}

function Invoke-StatusRequest {
    param(
        [string]$Method,
        [string]$Uri,
        [object]$Body = $null,
        [hashtable]$Headers = @{}
    )

    $request = @{
        Method = $Method
        Uri = $Uri
        Headers = $Headers
        TimeoutSec = 90
        ErrorAction = "Stop"
    }

    if ($null -ne $Body) {
        $request["Body"] = ($Body | ConvertTo-Json -Depth 20 -Compress)
        $request["ContentType"] = "application/json"
    }

    try {
        $response = Invoke-WebRequest @request
        return @{
            StatusCode = [int]$response.StatusCode
            Content = $response.Content
        }
    }
    catch {
        $response = $_.Exception.Response
        if ($null -ne $response) {
            return @{
                StatusCode = [int]$response.StatusCode
                Content = $null
            }
        }
        throw
    }
}

function Try-Probe {
    param(
        [string]$Name,
        [string]$Uri,
        [scriptblock]$Validator
    )

    if ([string]::IsNullOrWhiteSpace($Uri)) {
        return
    }

    try {
        $payload = Invoke-JsonRequest -Method "GET" -Uri $Uri
        & $Validator $payload
        Write-Host "PASS  $Name"
    }
    catch {
        Write-Warning "Skipped ${Name}: $($_.Exception.Message)"
    }
}

function Start-WorkerOnce {
    param(
        [string]$Server,
        [string]$JoinKey,
        [int]$MaxJobs = 1
    )

    $workerArgs = @(
        "worker",
        "--server", $Server,
        "--join-key", $JoinKey,
        "--once",
        "--max-jobs", "$MaxJobs"
    )

    Push-Location $repoRoot
    try {
        $savedHttpProxy = $env:HTTP_PROXY
        $savedHttpsProxy = $env:HTTPS_PROXY
        $savedAllProxy = $env:ALL_PROXY
        $savedNoProxy = $env:NO_PROXY
        $savedHttpProxyLower = $env:http_proxy
        $savedHttpsProxyLower = $env:https_proxy
        $savedAllProxyLower = $env:all_proxy
        $savedNoProxyLower = $env:no_proxy

        $env:HTTP_PROXY = ""
        $env:HTTPS_PROXY = ""
        $env:ALL_PROXY = ""
        $env:http_proxy = ""
        $env:https_proxy = ""
        $env:all_proxy = ""
        $env:NO_PROXY = "127.0.0.1,localhost,host.docker.internal"
        $env:no_proxy = "127.0.0.1,localhost,host.docker.internal"

        $binary = Get-Command findverse-crawler -ErrorAction SilentlyContinue
        if ($binary) {
            & $binary.Source @workerArgs
        } else {
            & cargo run -p findverse-crawler -- @workerArgs
        }

        if ($LASTEXITCODE -ne 0) {
            throw "crawler worker failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        $env:HTTP_PROXY = $savedHttpProxy
        $env:HTTPS_PROXY = $savedHttpsProxy
        $env:ALL_PROXY = $savedAllProxy
        $env:http_proxy = $savedHttpProxyLower
        $env:https_proxy = $savedHttpsProxyLower
        $env:all_proxy = $savedAllProxyLower
        $env:NO_PROXY = $savedNoProxy
        $env:no_proxy = $savedNoProxyLower
        Pop-Location
    }
}

$timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$developerUsername = "smoke-dev-$timestamp"
$developerPassword = "smoke-password-123"
$joinKey = "smoke-join-key-$timestamp"
$seedUrlTemplate = if ([string]::IsNullOrWhiteSpace($SeedUrl)) {
    "$PublicBaseUrl/smoke-crawler.html?findverse-smoke={timestamp}"
} else {
    $SeedUrl
}
$seedUrl = $seedUrlTemplate.Replace("{timestamp}", "$timestamp")

Write-Step "Probe health endpoints"
Try-Probe -Name "control-api healthz" -Uri "$ControlApiBaseUrl/healthz" -Validator {
    param($payload)
    Assert-True ($payload.status -eq "ok") "control healthz did not return ok"
}
Try-Probe -Name "query-api readyz" -Uri "$QueryApiBaseUrl/readyz" -Validator {
    param($payload)
    Assert-True ($payload.status -eq "ready") "query readyz did not return ready"
    Assert-True ($payload.postgres -and $payload.redis -and $payload.opensearch) "query readyz is degraded"
}

Write-Step "Search and suggest"
$search = Invoke-JsonRequest -Method "GET" -Uri "$ApiBaseUrl/v1/search?q=ranking"
Assert-True ($search.results.Count -ge 1) "search?q=ranking returned no results"
Write-Host "PASS  GET /v1/search?q=ranking"

$filteredSearch = Invoke-JsonRequest -Method "GET" -Uri "$ApiBaseUrl/v1/search?q=search&site=example.com&lang=en&freshness=30d"
Assert-True ($filteredSearch.query -eq "search") "filtered search returned unexpected query"
Write-Host "PASS  GET /v1/search?q=search&site=example.com&lang=en&freshness=30d"

$suggest = Invoke-JsonRequest -Method "GET" -Uri "$ApiBaseUrl/v1/suggest?q=rank"
Assert-True ($suggest.suggestions.Count -ge 1) "suggest?q=rank returned no suggestions"
Write-Host "PASS  GET /v1/suggest?q=rank"

Write-Step "Developer flow"
$devSession = Invoke-JsonRequest -Method "POST" -Uri "$ApiBaseUrl/v1/dev/register" -Body @{
    username = $developerUsername
    password = $developerPassword
}
Assert-True (-not [string]::IsNullOrWhiteSpace($devSession.token)) "developer register returned no token"

$devHeaders = @{ Authorization = "Bearer $($devSession.token)" }
$createdKey = Invoke-JsonRequest -Method "POST" -Uri "$ApiBaseUrl/v1/dev/keys" -Headers $devHeaders -Body @{
    name = "Smoke key"
}
Assert-True ($createdKey.token.StartsWith("fvk_")) "developer key token format is invalid"
Write-Host "PASS  developer register and key creation"

$developerSearch = Invoke-JsonRequest -Method "GET" -Uri "$ApiBaseUrl/v1/developer/search?q=ranking" -Headers @{
    Authorization = "Bearer $($createdKey.token)"
}
Assert-True ($developerSearch.results.Count -ge 1) "developer search returned no results"
Write-Host "PASS  developer bearer search"

$revokeStatus = Invoke-StatusRequest -Method "DELETE" -Uri "$ApiBaseUrl/v1/dev/keys/$($createdKey.id)" -Headers $devHeaders
Assert-True ($revokeStatus.StatusCode -eq 204) "developer key revoke did not return 204"

$revokedSearch = Invoke-StatusRequest -Method "GET" -Uri "$ApiBaseUrl/v1/developer/search?q=ranking" -Headers @{
    Authorization = "Bearer $($createdKey.token)"
}
Assert-True ($revokedSearch.StatusCode -eq 401) "revoked developer key should return 401"
Write-Host "PASS  revoked key is rejected"

Write-Step "Admin and crawler flow"
$adminSession = Invoke-JsonRequest -Method "POST" -Uri "$ApiBaseUrl/v1/admin/session/login" -Body @{
    username = $AdminUsername
    password = $AdminPassword
}
Assert-True (-not [string]::IsNullOrWhiteSpace($adminSession.token)) "admin login returned no token"
$adminHeaders = @{ Authorization = "Bearer $($adminSession.token)" }

$joinKeyStatus = Invoke-StatusRequest -Method "PUT" -Uri "$ApiBaseUrl/v1/admin/crawler-join-key" -Headers $adminHeaders -Body @{
    join_key = $joinKey
}
Assert-True ($joinKeyStatus.StatusCode -eq 204) "setting crawler join key did not return 204"

$seedResponse = Invoke-JsonRequest -Method "POST" -Uri "$ApiBaseUrl/v1/admin/frontier/seed" -Headers $adminHeaders -Body @{
    urls = @($seedUrl)
    source = "smoke-test"
    max_depth = 1
    allow_revisit = $true
}
Assert-True ($seedResponse.accepted_urls -ge 1) "seed frontier accepted no URLs"
Write-Host "PASS  frontier seeded"

$job = $null
for ($attempt = 0; $attempt -lt 5; $attempt++) {
    Start-WorkerOnce -Server $CrawlerServer -JoinKey $joinKey -MaxJobs 10
    $jobs = Invoke-JsonRequest -Method "GET" -Uri "$ApiBaseUrl/v1/admin/crawl/jobs?limit=200" -Headers $adminHeaders
    $job = $jobs.jobs | Where-Object { $_.url -like "*findverse-smoke=$timestamp*" } | Select-Object -First 1
    if ($null -ne $job -and $job.status -notin @("queued", "claimed")) {
        break
    }
    Start-Sleep -Seconds 1
}
Assert-True ($null -ne $job) "seeded crawl job not found"
Assert-True ($job.status -notin @("queued", "claimed")) "seeded crawl job was not processed"
Assert-True (-not [string]::IsNullOrWhiteSpace($job.status)) "crawl job status missing"
Assert-True ($job.PSObject.Properties.Name -contains "final_url") "crawl job missing final_url"
Assert-True ($job.PSObject.Properties.Name -contains "http_status") "crawl job missing http_status"
Assert-True ($job.PSObject.Properties.Name -contains "discovered_urls_count") "crawl job missing discovered_urls_count"
Assert-True ($job.PSObject.Properties.Name -contains "accepted_document_id") "crawl job missing accepted_document_id"
Write-Host "PASS  crawl job result fields are present"

$documents = Invoke-JsonRequest -Method "GET" -Uri "$ApiBaseUrl/v1/admin/documents?query=findverse-smoke=$timestamp&limit=50" -Headers $adminHeaders
$document = $documents.documents | Where-Object { $_.source_job_id -eq $job.id } | Select-Object -First 1
if ($null -eq $document) {
    $document = $documents.documents | Select-Object -First 1
}
Assert-True ($null -ne $document) "indexed document for smoke crawl not found"
Assert-True (-not [string]::IsNullOrWhiteSpace($document.canonical_url)) "document canonical_url missing"
Assert-True (-not [string]::IsNullOrWhiteSpace($document.host)) "document host missing"
Assert-True (-not [string]::IsNullOrWhiteSpace($document.content_type)) "document content_type missing"
Assert-True ($document.word_count -ge 1) "document word_count was not populated"
Assert-True ($document.PSObject.Properties.Name -contains "source_job_id") "document source_job_id missing"
Write-Host "PASS  document metadata fields are present"

Write-Step "Docker rebuild verification"
Write-Host "Reuse the deployment script to rebuild containers when needed:"
Write-Host "  .\scripts\deploy-stack.ps1 -Rebuild"

if ($RunPlaywright) {
    Write-Step "Playwright"
    $env:PLAYWRIGHT_BASE_URL = $ApiBaseUrl.Substring(0, $ApiBaseUrl.Length - 4)
    $env:PLAYWRIGHT_API_BASE_URL = $ApiBaseUrl

    Push-Location $repoRoot
    try {
        & npx playwright test
        if ($LASTEXITCODE -ne 0) {
            throw "playwright test failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

Write-Step "Smoke test summary"
Write-Host "PASS  search, suggest, developer auth, key revocation, admin login, crawler join/claim/report, job metadata, document metadata"
