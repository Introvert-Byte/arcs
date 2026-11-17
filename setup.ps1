# Convenience wrapper — run from project root on Windows
param([switch]$Run)
& "$PSScriptRoot\scripts\setup.ps1" -Run:$Run
