<#
.SYNOPSIS
  Build the Windows installer on the build machine.

.DESCRIPTION
  The Windows release ships webby's own build of WebKit (webkit/patches),
  which is built locally and not in CI. This collects that build and
  webby.exe into a staging folder and compiles the installer from it.

  Needs a finished WebKit build (see webkit/NOTES.md) and Inno Setup 6
  (winget install JRSoftware.InnoSetup).

.EXAMPLE
  .\scripts\package-windows.ps1
  .\scripts\package-windows.ps1 -WebKitBuild D:\wk-build\Release -StageOnly
#>
param(
    # The WebKit build directory that holds bin\ and vcpkg_installed\.
    [string]$WebKitBuild = "C:\wk-build-gst\Release",
    [string]$Stage = "stage",
    # Stop after staging: a folder webby.exe runs from, without an installer.
    [switch]$StageOnly
)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot)

$bin = Join-Path $WebKitBuild "bin"
$deps = Join-Path $WebKitBuild "vcpkg_installed\x64-windows-webkit"
if (-not (Test-Path (Join-Path $bin "WebKit2.dll"))) { throw "No WebKit2.dll in $bin. Build WebKit first, or pass -WebKitBuild." }
if (-not (Test-Path (Join-Path $deps "plugins\gstreamer"))) { throw "No GStreamer plugins under $deps. This is not a build made with webkit/patches." }

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
Write-Host "webby $version"

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed." }

if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Path $Stage, "$Stage\gstreamer-1.0", "$Stage\licenses" | Out-Null

# WebKit and its helper processes, without debug files and test programs.
$skip = '\.(pdb|ilk|exp|lib)$|^(test|LLInt|TestWebKitAPI|WebKitTestRunner|DumpRenderTree|MiniBrowser|WebDriver)|^jsc\.exe$'
Get-ChildItem $bin | Where-Object { $_.Name -notmatch $skip } | Copy-Item -Destination $Stage -Recurse
# The libraries WebKit was built against: GStreamer, GLib, FFmpeg and what they need.
Copy-Item (Join-Path $deps "bin\*.dll") $Stage -Force
# GStreamer's plugins and the helper that scans them. webby looks for this folder.
Copy-Item (Join-Path $deps "plugins\gstreamer\*.dll") "$Stage\gstreamer-1.0"
Copy-Item (Join-Path $deps "tools\gstreamer\gst-plugin-scanner.exe") $Stage
# The licence of every bundled library.
Get-ChildItem (Join-Path $deps "share") -Directory | ForEach-Object {
    $copyright = Join-Path $_.FullName "copyright"
    if (Test-Path $copyright) { Copy-Item $copyright "$Stage\licenses\$($_.Name).txt" }
}
Copy-Item target\release\webby.exe $Stage

$size = [math]::Round((Get-ChildItem $Stage -Recurse -File | Measure-Object Length -Sum).Sum / 1MB)
Write-Host "Staged $((Get-ChildItem $Stage -File).Count) files, $((Get-ChildItem "$Stage\gstreamer-1.0").Count) plugins, $size MB in $Stage"
if ($StageOnly) { return }

$iscc = @("$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw "Inno Setup 6 is not installed. Run: winget install JRSoftware.InnoSetup" }
& $iscc "/Q" "/DVersion=$version" "/DSourceDir=$((Resolve-Path $Stage).Path)" packaging\windows\webby.iss
if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed." }
New-Item -ItemType Directory -Path dist -Force | Out-Null
Move-Item "dist-installer\webby-$version-windows-x64-setup.exe" dist\ -Force
Write-Host "Installer: $((Resolve-Path "dist\webby-$version-windows-x64-setup.exe").Path)"
