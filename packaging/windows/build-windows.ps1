[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$Install,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

if ($Help) {
    Write-Host "Usage: .\build-windows.ps1 [-SkipBuild] [-Install] [-Help]"
    Write-Host "Builds binaries, packages portable zip, and compiles Inno Setup installer."
    exit 0
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PackagingDir = Split-Path -Parent $ScriptDir
$RootDir = Split-Path -Parent $PackagingDir
$DistDir = Join-Path $RootDir "dist"

Write-Host "`n========================================================" -ForegroundColor Cyan
Write-Host " [UWU-LOG] Windows Packaging & Installer Builder" -ForegroundColor Cyan
Write-Host "========================================================" -ForegroundColor Cyan

# 1. Read Version from crates/ui_gui/Cargo.toml
$guiCargoPath = Join-Path $RootDir "crates\ui_gui\Cargo.toml"
$versionMatch = Get-Content $guiCargoPath -ErrorAction SilentlyContinue | Select-String -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
$version = if ($versionMatch) { $versionMatch.Matches.Groups[1].Value } else { "0.1.0" }

Write-Host "   -> Package Version: $version" -ForegroundColor Green

# 2. Build Release Binaries
if (-not $SkipBuild) {
    Write-Host "`n[1/4] Building release binaries (uwu-gui, uwu-tui, uwu-agent, uwulog)..." -ForegroundColor Yellow
    Push-Location $RootDir
    cargo build --release --bin uwu-gui --bin uwu-tui --bin uwu-agent --bin uwulog
    Pop-Location
} else {
    Write-Host "`n[1/4] Skipping cargo build (-SkipBuild specified)..." -ForegroundColor Gray
}

# 3. Create dist directory
if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
}

# 4. Code Signing (Optional Hook)
$canSign = $false
$signtool = Get-Command "signtool.exe" -ErrorAction SilentlyContinue

if ($env:WINDOWS_SIGN_CERT -and (Test-Path $env:WINDOWS_SIGN_CERT)) {
    if ($signtool) {
        $canSign = $true
        Write-Host "`n[2/4] Code signing certificate detected. Signing binaries..." -ForegroundColor Yellow
        $binariesToSign = @(
            (Join-Path $RootDir "target\release\uwu-gui.exe"),
            (Join-Path $RootDir "target\release\uwu-tui.exe"),
            (Join-Path $RootDir "target\release\uwu-agent.exe"),
            (Join-Path $RootDir "target\release\uwulog.exe")
        )
        foreach ($bin in $binariesToSign) {
            Write-Host "      -> Signing $bin"
            & $signtool sign /f "$env:WINDOWS_SIGN_CERT" /p "$env:WINDOWS_SIGN_PASS" /tr http://timestamp.digicert.com /td sha256 /fd sha256 "$bin"
        }
    } else {
        Write-Warning "signtool.exe not found in PATH. Skipping signature."
    }
} else {
    Write-Host "`n[2/4] Code signing: No certificate provided (Build unsigned for local test)" -ForegroundColor Gray
}

# 5. Build Portable ZIP
Write-Host "`n[3/4] Creating portable ZIP package..." -ForegroundColor Yellow
$zipTempDir = Join-Path $DistDir "uwulog-v$version-windows-x64"
if (Test-Path $zipTempDir) {
    Remove-Item -Recurse -Force $zipTempDir
}
New-Item -ItemType Directory -Force -Path (Join-Path $zipTempDir "bin") | Out-Null

Copy-Item -Force (Join-Path $RootDir "target\release\uwu-gui.exe") $zipTempDir
Copy-Item -Force (Join-Path $RootDir "target\release\uwu-tui.exe") $zipTempDir
Copy-Item -Force (Join-Path $RootDir "target\release\uwu-agent.exe") $zipTempDir
Copy-Item -Force (Join-Path $RootDir "packaging\assets\icon.ico") $zipTempDir
Copy-Item -Force (Join-Path $RootDir "target\release\uwulog.exe") (Join-Path $zipTempDir "bin\uwulog.exe")
Copy-Item -Force (Join-Path $ScriptDir "bin\*") (Join-Path $zipTempDir "bin")

$zipOutPath = Join-Path $DistDir "uwulog-v$version-windows-x64.zip"
if (Test-Path $zipOutPath) {
    Remove-Item -Force $zipOutPath
}
Compress-Archive -Path "$zipTempDir\*" -DestinationPath $zipOutPath -Force
Remove-Item -Recurse -Force $zipTempDir
Write-Host "   -> [OK] Portable ZIP created: $zipOutPath" -ForegroundColor Green

# 6. Compile Inno Setup Installer
Write-Host "`n[4/4] Compiling Inno Setup installer..." -ForegroundColor Yellow

$isccCandidates = @(
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe",
    "ISCC.exe"
)

$isccExe = $null
foreach ($candidate in $isccCandidates) {
    if (Test-Path $candidate) {
        $isccExe = $candidate
        break
    } elseif (Get-Command $candidate -ErrorAction SilentlyContinue) {
        $isccExe = (Get-Command $candidate).Source
        break
    }
}

if (-not $isccExe) {
    Write-Error "Inno Setup Compiler (ISCC.exe) not found! Install it via: winget install JRSoftware.InnoSetup"
    exit 1
}

Write-Host "   -> Using Inno Setup: $isccExe" -ForegroundColor Gray
$issPath = Join-Path $ScriptDir "setup.iss"

& $isccExe "/DAppVersion=$version" "/DSourceDir=$RootDir" "/DOutputDir=$DistDir" "$issPath"

$installerPath = Join-Path $DistDir "uwulog-setup-x64.exe"
if (Test-Path $installerPath) {
    if ($canSign) {
        Write-Host "   -> Signing installer: $installerPath" -ForegroundColor Yellow
        & $signtool sign /f "$env:WINDOWS_SIGN_CERT" /p "$env:WINDOWS_SIGN_PASS" /tr http://timestamp.digicert.com /td sha256 /fd sha256 "$installerPath"
    }

    Write-Host "`n========================================================" -ForegroundColor Green
    Write-Host "  BUILD & PACKAGING COMPLETED SUCCESSFULLY!" -ForegroundColor Green
    Write-Host "========================================================" -ForegroundColor Green
    Write-Host "Generated Artifacts in dist/:" -ForegroundColor White
    Write-Host "   1. Setup Wizard:  $installerPath" -ForegroundColor Cyan
    Write-Host "   2. Portable ZIP:  $zipOutPath" -ForegroundColor Cyan
    Write-Host ""

    if ($Install) {
        Write-Host "Launching installer as requested (-Install)..." -ForegroundColor Yellow
        Start-Process -FilePath $installerPath
    }
} else {
    Write-Error "Installer generation failed. $installerPath was not created."
    exit 1
}
