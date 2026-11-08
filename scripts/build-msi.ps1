# PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
# Build the Windows installer of Kshana Pro (roadmap L5) with WiX Toolset v4:
#
#   dotnet tool install --global wix --version 4.0.5     # once
#   scripts/build-msi.ps1 -Binary target\release\kshana-pro.exe -Out dist
#
# Writes <Out>\kshana-pro-<version>-x64.msi and prints its path. The MSI version is the
# crate version without a pre-release part (Windows Installer versions are numeric). The
# package is unsigned; signing waits for the publisher's certificate.
param(
  [Parameter(Mandatory = $true)][string]$Binary,
  [Parameter(Mandatory = $true)][string]$Out
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$bin = (Resolve-Path $Binary).Path
$line = Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1
if (-not $line) { throw 'no version in Cargo.toml' }
$version = ($line.Matches[0].Groups[1].Value -split '-')[0]
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$msi = Join-Path (Resolve-Path $Out).Path "kshana-pro-$version-x64.msi"
& wix build -arch x64 `
  -d "Version=$version" -d "Binary=$bin" -d "Root=$root" `
  -o $msi (Join-Path $root 'deploy\windows\kshana-pro.wxs')
if ($LASTEXITCODE -ne 0) { throw "wix build failed ($LASTEXITCODE)" }
Write-Output $msi
