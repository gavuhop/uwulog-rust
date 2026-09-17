[CmdletBinding()]
param(
    [ValidateSet("x64", "arm64", "x86_64", "aarch64", "auto")]
    [string]$Architecture = "auto",
    [string]$TargetTriple,
    [switch]$SkipBuild,
    [switch]$Install,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

if ($Help) {
    Write-Host "Usage: .\build-windows.ps1 [-Architecture <x64|arm64|auto>] [-TargetTriple <triple>] [-SkipBuild] [-Install] [-Help]"
    Write-Host "Builds binaries, packages portable zip, and compiles Inno Setup installer for the target architecture."
    exit 0
}

# Normalize architecture to x64 or arm64
$osArch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLower()
if ($Architecture -eq "auto") {
    $Architecture = if ($osArch -eq "arm64") { "arm64" } else { "x64" }
} elseif ($Architecture -eq "x86_64") {
    $Architecture = "x64"
} elseif ($Architecture -eq "aarch64") {
    $Architecture = "arm64"
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PackagingDir = Split-Path -Parent $ScriptDir
$RootDir = Split-Path -Parent $PackagingDir
$DistDir = Join-Path $RootDir "dist"

Write-Host "`n========================================================" -ForegroundColor Cyan
Write-Host " [UWU-LOG] Windows Packaging & Installer Builder" -ForegroundColor Cyan
Write-Host " Architecture: $Architecture (Host OS: $osArch)" -ForegroundColor Cyan
Write-Host "========================================================" -ForegroundColor Cyan

# 1. Read Version from crates/ui_gui/Cargo.toml
$guiCargoPath = Join-Path $RootDir "crates\ui_gui\Cargo.toml"
$versionMatch = Get-Content $guiCargoPath -ErrorAction SilentlyContinue | Select-String -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
$version = if ($versionMatch) { $versionMatch.Matches.Groups[1].Value } else { "0.1.0" }

Write-Host "   -> Package Version: $version" -ForegroundColor Green

# 2. Determine Cargo Target and Binary Directory
$cargoTargetArgs = @()
if ($TargetTriple) {
    $cargoTargetArgs = @("--target", $TargetTriple)
    $binDir = Join-Path $RootDir "target\$TargetTriple\release"
} elseif ($Architecture -eq "arm64" -and $osArch -ne "arm64") {
    $TargetTriple = "aarch64-pc-windows-msvc"
    $cargoTargetArgs = @("--target", $TargetTriple)
    $binDir = Join-Path $RootDir "target\$TargetTriple\release"
} elseif ($Architecture -eq "x64" -and $osArch -eq "arm64") {
    $TargetTriple = "x86_64-pc-windows-msvc"
    $cargoTargetArgs = @("--target", $TargetTriple)
    $binDir = Join-Path $RootDir "target\$TargetTriple\release"
} else {
    $binDir = Join-Path $RootDir "target\release"
}

# 3. Build Release Binaries
if (-not $SkipBuild) {
    Write-Host "`n[1/4] Building release binaries for $Architecture (uwu-gui, uwu-tui, uwu-agent, uwulog)..." -ForegroundColor Yellow
    Push-Location $RootDir
    cargo build --release @cargoTargetArgs --bin uwu-gui --bin uwu-tui --bin uwu-agent --bin uwulog
    Pop-Location
} else {
    Write-Host "`n[1/4] Skipping cargo build (-SkipBuild specified)..." -ForegroundColor Gray
    # Graceful fallback if binDir doesn't exist yet but target\release does
    if (-not (Test-Path (Join-Path $binDir "uwu-gui.exe")) -and (Test-Path (Join-Path $RootDir "target\release\uwu-gui.exe"))) {
        $binDir = Join-Path $RootDir "target\release"
    }
}

Write-Host "   -> Using binary directory: $binDir" -ForegroundColor Gray

# Check for optional bundled prebuilt Linux agents for WSL provisioning
$linuxAgentX64 = Join-Path $RootDir "agents\uwu-agent-x86_64"
$linuxAgentArm64 = Join-Path $RootDir "agents\uwu-agent-aarch64"
if (Test-Path $linuxAgentX64) {
    Write-Host "   -> Detected bundled Linux Agent (x86_64): $linuxAgentX64" -ForegroundColor Cyan
}
if (Test-Path $linuxAgentArm64) {
    Write-Host "   -> Detected bundled Linux Agent (aarch64): $linuxAgentArm64" -ForegroundColor Cyan
}

# 4. Create dist directory
if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
}

# 5. Code Signing (Optional Hook)
$canSign = $false
$signtool = Get-Command "signtool.exe" -ErrorAction SilentlyContinue

if ($env:WINDOWS_SIGN_CERT -and (Test-Path $env:WINDOWS_SIGN_CERT)) {
    if ($signtool) {
        $canSign = $true
        Write-Host "`n[2/4] Code signing certificate detected. Signing binaries..." -ForegroundColor Yellow
        $binariesToSign = @(
            (Join-Path $binDir "uwu-gui.exe"),
            (Join-Path $binDir "uwu-tui.exe"),
            (Join-Path $binDir "uwu-agent.exe"),
            (Join-Path $binDir "uwulog.exe")
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

# 6. Build Portable ZIP
Write-Host "`n[3/4] Creating portable ZIP package for $Architecture..." -ForegroundColor Yellow
$zipTempDir = Join-Path $DistDir "uwulog-v$version-windows-$Architecture"
if (Test-Path $zipTempDir) {
    Remove-Item -Recurse -Force $zipTempDir
}
New-Item -ItemType Directory -Force -Path (Join-Path $zipTempDir "bin") | Out-Null

Copy-Item -Force (Join-Path $binDir "uwu-gui.exe") $zipTempDir
Copy-Item -Force (Join-Path $binDir "uwu-tui.exe") $zipTempDir
Copy-Item -Force (Join-Path $binDir "uwu-agent.exe") $zipTempDir
Copy-Item -Force (Join-Path $RootDir "packaging\assets\icon.ico") $zipTempDir
Copy-Item -Force (Join-Path $binDir "uwulog.exe") (Join-Path $zipTempDir "bin\uwulog.exe")
Copy-Item -Force (Join-Path $ScriptDir "bin\*") (Join-Path $zipTempDir "bin")

$zipOutPath = Join-Path $DistDir "uwulog-v$version-windows-$Architecture.zip"
if (Test-Path $zipOutPath) {
    Remove-Item -Force $zipOutPath
}
Compress-Archive -Path "$zipTempDir\*" -DestinationPath $zipOutPath -Force
Remove-Item -Recurse -Force $zipTempDir
Write-Host "   -> [OK] Portable ZIP created: $zipOutPath" -ForegroundColor Green

# 7. Compile Inno Setup Installer
Write-Host "`n[4/4] Compiling Inno Setup installer for $Architecture..." -ForegroundColor Yellow

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

& $isccExe "/DAppVersion=$version" "/DAppArch=$Architecture" "/DSourceDir=$RootDir" "/DOutputDir=$DistDir" "/DBinDir=$binDir" "$issPath"

$installerPath = Join-Path $DistDir "uwulog-setup-$Architecture.exe"
if (Test-Path $installerPath) {
    if ($canSign) {
        Write-Host "   -> Signing installer: $installerPath" -ForegroundColor Yellow
        & $signtool sign /f "$env:WINDOWS_SIGN_CERT" /p "$env:WINDOWS_SIGN_PASS" /tr http://timestamp.digicert.com /td sha256 /fd sha256 "$installerPath"
    }

    Write-Host "`n========================================================" -ForegroundColor Green
    Write-Host "  BUILD & PACKAGING COMPLETED SUCCESSFULLY!" -ForegroundColor Green
    Write-Host "========================================================" -ForegroundColor Green
    Write-Host "Generated Artifacts in dist/:" -ForegroundColor White
    Write-Host "   1. Setup Wizard ($Architecture):  $installerPath" -ForegroundColor Cyan
    Write-Host "   2. Portable ZIP ($Architecture):  $zipOutPath" -ForegroundColor Cyan
    Write-Host ""

    if ($Install) {
        Write-Host "Launching installer as requested (-Install)..." -ForegroundColor Yellow
        Start-Process -FilePath $installerPath
    }
} else {
    Write-Error "Installer generation failed. $installerPath was not created."
    exit 1
}
