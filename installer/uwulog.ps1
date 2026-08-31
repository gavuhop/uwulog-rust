# PowerShell wrapper for uwulog
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
& "$ScriptDir\uwu-gui.exe" @args
