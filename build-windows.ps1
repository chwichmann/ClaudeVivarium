# Build Claude Vivarium for Windows into dist\ClaudeVivarium\ and optionally install it.
#   .\build-windows.ps1            build only
#   .\build-windows.ps1 -Install   build and install for the current user (no admin rights needed)
param([switch]$Install)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if (-not (Test-Path .venv\Scripts\python.exe)) {
    python -m venv .venv
}
.venv\Scripts\python -m pip install -q --disable-pip-version-check pycairo pyinstaller

$hidden = "theme_aquarium","theme_space","theme_synthwave","theme_pasture","theme_seabed","theme_forest","theme_guppies","theme_terrarium","theme_meerkats" |
    ForEach-Object { "--hidden-import=$_" }
.venv\Scripts\pyinstaller --noconfirm --clean --onedir --noconsole --name ClaudeVivarium `
    --distpath dist --workpath build $hidden vivarium_win.py
if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed" }
Copy-Item dist\ClaudeVivarium\ClaudeVivarium.exe dist\ClaudeVivarium\ClaudeVivarium.scr -Force
Write-Host "Built: dist\ClaudeVivarium\ClaudeVivarium.scr"

if ($Install) {
    & dist\ClaudeVivarium\ClaudeVivarium.exe --install | Out-Host
}
