# Arma la versión pública de Relics Reforged para publicar (Nexus Mods).
#
# Compila la DLL sin la feature "dev" en target/public (así no pisa la de desarrollo de
# target/release, que usa me3/relics-reforged-dev.me3) y deja en target/dist:
#   RelicsReforged/            carpeta lista para probar (se le puede copiar SeamlessCoop adentro)
#   RelicsReforged-v<ver>.zip  lo que se sube
#
# Uso, desde la raíz del repositorio:  powershell -File tools/package.ps1

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host 'Compilando la DLL publica...'
cargo build --release -p relics-reforged --target-dir target/public
if ($LASTEXITCODE -ne 0) { throw 'cargo build fallo' }

$meta = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$version = ($meta.packages | Where-Object { $_.name -eq 'relics-reforged' }).version

$dll = Join-Path $root 'target/public/release/relics_reforged.dll'
# La versión pública no puede llevar las herramientas de desarrollo.
if (Select-String -Path $dll -Pattern 'relics_reforged_cmd.txt' -SimpleMatch -Quiet) {
    throw 'La DLL tiene el archivo de comandos: se compilo con la feature dev'
}

$dist = Join-Path $root 'target/dist'
$folder = Join-Path $dist 'RelicsReforged'
if (Test-Path $folder) { Remove-Item -Recurse -Force $folder }
New-Item -ItemType Directory -Force $folder | Out-Null

Copy-Item $dll $folder
Copy-Item (Join-Path $root 'dist/*') $folder
Copy-Item (Join-Path $root 'LICENSE') $folder

$zip = Join-Path $dist "RelicsReforged-v$version.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $folder -DestinationPath $zip

Write-Host ''
Write-Host "Listo: $zip"
Get-ChildItem $folder | Format-Table Name, Length -AutoSize
