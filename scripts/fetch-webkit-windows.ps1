<#
.SYNOPSIS
  Download a prebuilt WebKit for Windows and place it next to webby.exe.

.DESCRIPTION
  webby on Windows loads WebKit2.dll at startup. Apple does not publish
  Windows binaries of WebKit, so by default this fetches the build of
  WebKit's Windows port that the Playwright project publishes for browser
  testing. Set WEBBY_WEBKIT_URL to use any other zip that contains
  WebKit2.dll, for example one you built from webkit.org sources.

.EXAMPLE
  cargo build --release
  .\scripts\fetch-webkit-windows.ps1 -Destination target\release
#>
param(
    [string]$Destination = "target\release",
    [string]$Revision
)
$ErrorActionPreference = "Stop"

$url = $env:WEBBY_WEBKIT_URL
if (-not $url) {
    if (-not $Revision) {
        $manifest = Invoke-RestMethod "https://raw.githubusercontent.com/microsoft/playwright/main/packages/playwright-core/browsers.json"
        $Revision = ($manifest.browsers | Where-Object { $_.name -eq "webkit" }).revision
    }
    $url = "https://cdn.playwright.dev/dbazure/download/playwright/builds/webkit/$Revision/webkit-win64.zip"
}

$work = Join-Path ([IO.Path]::GetTempPath()) "webby-webkit-$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Write-Host "Downloading $url"
    $zip = Join-Path $work "webkit.zip"
    Invoke-WebRequest -Uri $url -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath (Join-Path $work "unpacked")

    $dll = Get-ChildItem -Path (Join-Path $work "unpacked") -Recurse -Filter "WebKit2.dll" | Select-Object -First 1
    if (-not $dll) {
        throw "The archive does not contain WebKit2.dll, so it is not a build of WebKit's Windows port."
    }
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    Copy-Item -Path (Join-Path $dll.DirectoryName "*") -Destination $Destination -Recurse -Force
    Write-Host "WebKit installed in $Destination"
}
finally {
    Remove-Item -Recurse -Force $work
}
