# PowerShell wrapper for uwulog
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
if ($args.Count -gt 0) {
    Start-Process -FilePath "$ScriptDir\uwu-gui.exe" -ArgumentList $args
} else {
    Start-Process -FilePath "$ScriptDir\uwu-gui.exe"
}
