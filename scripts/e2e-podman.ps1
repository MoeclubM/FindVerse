$ErrorActionPreference = "Stop"

$env:AUTH_SECRET = if ($env:AUTH_SECRET) { $env:AUTH_SECRET } else { "findverse-local-secret" }
$env:FINDVERSE_LOCAL_ADMIN_USERNAME = if ($env:FINDVERSE_LOCAL_ADMIN_USERNAME) { $env:FINDVERSE_LOCAL_ADMIN_USERNAME } else { "admin" }
$env:FINDVERSE_LOCAL_ADMIN_PASSWORD = if ($env:FINDVERSE_LOCAL_ADMIN_PASSWORD) { $env:FINDVERSE_LOCAL_ADMIN_PASSWORD } else { "change-me" }
$env:FINDVERSE_API_PORT = if ($env:FINDVERSE_API_PORT) { $env:FINDVERSE_API_PORT } else { "8081" }
$env:FINDVERSE_WEB_PORT = if ($env:FINDVERSE_WEB_PORT) { $env:FINDVERSE_WEB_PORT } else { "3100" }
$env:PLAYWRIGHT_BASE_URL = if ($env:PLAYWRIGHT_BASE_URL) { $env:PLAYWRIGHT_BASE_URL } else { "http://127.0.0.1:$($env:FINDVERSE_WEB_PORT)" }
$env:PLAYWRIGHT_API_BASE_URL = if ($env:PLAYWRIGHT_API_BASE_URL) { $env:PLAYWRIGHT_API_BASE_URL } else { "http://127.0.0.1:$($env:FINDVERSE_API_PORT)" }

function Reset-PodmanProxyEnv {
  podman machine ssh @'
systemctl --user stop podman.service podman.socket
systemctl --user unset-environment HTTP_PROXY HTTPS_PROXY NO_PROXY http_proxy https_proxy no_proxy
systemctl --user set-environment HTTP_PROXY= HTTPS_PROXY= NO_PROXY= http_proxy= https_proxy= no_proxy=
systemctl --user daemon-reexec
systemctl --user start podman.socket
'@ | Out-Null
}

function Invoke-ExternalChecked($Command, $FailureMessage) {
  & $Command
  if ($LASTEXITCODE -ne 0) {
    throw $FailureMessage
  }
}

function Wait-Http($Url, $Attempts = 120) {
  for ($i = 0; $i -lt $Attempts; $i++) {
    try {
      Invoke-WebRequest -Uri $Url -UseBasicParsing | Out-Null
      return
    } catch {
      Start-Sleep -Seconds 1
    }
  }

  throw "Timed out waiting for $Url"
}

Reset-PodmanProxyEnv
Invoke-ExternalChecked { podman pull docker.io/library/node:24-alpine } "Failed to pull node base image"
Invoke-ExternalChecked { podman pull docker.io/library/rust:1.93-bookworm } "Failed to pull rust base image"
Invoke-ExternalChecked { podman pull gcr.io/distroless/cc-debian12 } "Failed to pull distroless base image"
Invoke-ExternalChecked { podman compose down -v } "Failed to stop the existing Podman stack"
Invoke-ExternalChecked { podman compose up --build -d api web } "Failed to build or start the Podman stack"

try {
  Wait-Http "$($env:PLAYWRIGHT_API_BASE_URL)/healthz"
  Wait-Http "$($env:PLAYWRIGHT_BASE_URL)/developers"
  Invoke-ExternalChecked { npx playwright test } "Playwright end-to-end suite failed"
} finally {
  podman compose down -v | Out-Null
}
