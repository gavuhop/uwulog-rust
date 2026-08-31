# ==============================================================================
# install.ps1 - Automated Installer for Uwu Log Viewer (Windows & WSL)
# ==============================================================================

$ErrorActionPreference = "Stop"

Write-Host "====================================================" -ForegroundColor Cyan
Write-Host " [UWU-LOG] Starting Installation for Windows & WSL " -ForegroundColor Cyan
Write-Host "====================================================" -ForegroundColor Cyan

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $ScriptDir) {
    $ScriptDir = (Get-Location).Path
}
$RootDir = Split-Path -Parent $ScriptDir

# 1. Build Release Binaries
Write-Host "`n[1/4] Building release binaries with cargo..." -ForegroundColor Yellow
Set-Location $RootDir
cargo build --release

# 2. Setup install directory
$localAppData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
$InstallDir = Join-Path $localAppData "Programs\uwulog\bin"

Write-Host "`n[2/4] Installing binaries to: $InstallDir" -ForegroundColor Yellow
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

# Dừng các tiến trình đang chạy nếu có để tránh lock file khi copy
Stop-Process -Name "uwu-gui", "uwulog", "uwu-tui", "uwu-agent" -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

Copy-Item -Force (Join-Path $RootDir "target\release\uwu-gui.exe") (Join-Path $InstallDir "uwu-gui.exe")
Copy-Item -Force (Join-Path $RootDir "target\release\uwu-gui.exe") (Join-Path $InstallDir "uwulog.exe")
Copy-Item -Force (Join-Path $RootDir "target\release\uwu-tui.exe") (Join-Path $InstallDir "uwu-tui.exe")
Copy-Item -Force (Join-Path $RootDir "target\release\uwu-agent.exe") (Join-Path $InstallDir "uwu-agent.exe")
Copy-Item -Force (Join-Path $ScriptDir "uwulog.cmd") (Join-Path $InstallDir "uwulog.cmd")
Copy-Item -Force (Join-Path $ScriptDir "uwulog.cmd") (Join-Path $InstallDir "uwulog.bat")
Copy-Item -Force (Join-Path $ScriptDir "uwulog.ps1") (Join-Path $InstallDir "uwulog.ps1")
Copy-Item -Force (Join-Path $ScriptDir "uwulog") (Join-Path $InstallDir "uwulog")

Write-Host "   -> [OK] Binaries and launchers copied successfully." -ForegroundColor Green

# 3. Configure Windows User PATH
Write-Host "`n[3/4] Configuring Windows User PATH..." -ForegroundColor Yellow
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    $NewPath = ($UserPath.TrimEnd(';') + ";$InstallDir").Trim(';')
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    Write-Host "   -> [OK] Added '$InstallDir' to Windows User PATH!" -ForegroundColor Green
} else {
    Write-Host "   -> [INFO] '$InstallDir' already exists in Windows User PATH." -ForegroundColor Gray
}

# Cập nhật PATH cho phiên làm việc hiện tại
$env:Path = [Environment]::GetEnvironmentVariable("Path", "User") + ";" + [Environment]::GetEnvironmentVariable("Path", "Machine")

# 4. Configure WSL Distros
Write-Host "`n[4/4] Detecting and configuring WSL Distros..." -ForegroundColor Yellow
try {
    $wslExe = Get-Command "wsl.exe" -ErrorAction Stop
    $rawDistros = & $wslExe --list --quiet 2>$null
    if ($rawDistros) {
        $Distros = $rawDistros | ForEach-Object { $_.Trim().Replace("`0", "") } | Where-Object { $_ -ne "" }
        foreach ($distro in $Distros) {
            Write-Host "   -> Configuring WSL distro: $distro" -ForegroundColor Cyan
            
            # Create ~/.local/bin
            & $wslExe -d $distro -- sh -c "mkdir -p ~/.local/bin ~/.local/share/uwu/server_state"
            
            # Copy script uwulog via wslpath
            $wslScriptPath = (Join-Path $InstallDir "uwulog").Replace('\', '/')
            $wslUnixSrc = (& $wslExe -d $distro -- wslpath -u "$wslScriptPath").Trim()
            & $wslExe -d $distro -- sh -c "cp '$wslUnixSrc' ~/.local/bin/uwulog && chmod 755 ~/.local/bin/uwulog"
            
            # Ensure PATH in ~/.bashrc
            $bashrcCheck = 'if ! grep -q ".local/bin" ~/.bashrc 2>/dev/null; then echo ''export PATH="$HOME/.local/bin:$PATH"'' >> ~/.bashrc; fi'
            & $wslExe -d $distro -- sh -c $bashrcCheck
            
            Write-Host "      -> [OK] Installed ~/.local/bin/uwulog in [$distro]" -ForegroundColor Green
        }
    } else {
        Write-Host "   -> [INFO] No WSL distros found." -ForegroundColor Gray
    }
} catch {
    Write-Host "   -> [INFO] WSL not available on this machine." -ForegroundColor Gray
}

Write-Host "`n====================================================" -ForegroundColor Green
Write-Host "  INSTALLATION COMPLETED SUCCESSFULLY! " -ForegroundColor Green
Write-Host "====================================================" -ForegroundColor Green
Write-Host "You can now open any terminal and run:" -ForegroundColor White
Write-Host "   * Windows (PowerShell/CMD): uwulog" -ForegroundColor Cyan
Write-Host "   * WSL (Ubuntu/Debian):      uwulog -j  OR  uwulog -f /var/log/syslog" -ForegroundColor Cyan
Write-Host ""
