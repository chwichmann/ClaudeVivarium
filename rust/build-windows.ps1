# Build the Rust version of Claude Vivarium for Windows and optionally install it.
#   .\build-windows.ps1            build only  ->  dist\ClaudeVivariumRS.scr
#   .\build-windows.ps1 -Install   build and install for the current user (no admin rights needed)
# Needs Rust (https://rustup.rs) with the Visual Studio C++ build tools.
param([switch]$Install)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item target\release\claude-vivarium-rs.exe dist\ClaudeVivariumRS.exe -Force
Copy-Item target\release\claude-vivarium-rs.exe dist\ClaudeVivariumRS.scr -Force
Write-Host "Built: dist\ClaudeVivariumRS.scr"

if ($Install) {
    Start-Process -Wait dist\ClaudeVivariumRS.exe -ArgumentList "--install"
}
