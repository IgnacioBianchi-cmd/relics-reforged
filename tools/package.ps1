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
# - crt-static: el runtime de C va dentro de la DLL, así no hace falta tener instalado el
#   Visual C++ Redistributable (el juego no lo usa, así que no se puede dar por hecho).
# - remap-path-prefix: los mensajes de panic llevan la ruta de cada archivo fuente; sin esto
#   quedan las rutas de esta PC (con el usuario de Windows) dentro de la DLL.
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' }
# Separados con 0x1F (CARGO_ENCODED_RUSTFLAGS): RUSTFLAGS corta en los espacios de las rutas.
$env:CARGO_ENCODED_RUSTFLAGS = @(
    '-Ctarget-feature=+crt-static',
    "--remap-path-prefix=$cargoHome=cargo",
    "--remap-path-prefix=$rustupHome=rustup",
    "--remap-path-prefix=$root=relics-reforged"
) -join [char]0x1F
# Lo mismo para el código C/C++ de las dependencias (imgui, zstd): sus aserciones llevan
# __FILE__. /d1trimfile le saca el prefijo (opción de cl.exe sin documentar, VS 2019+).
$env:CFLAGS = "/d1trimfile:$cargoHome\registry\src\"
$env:CXXFLAGS = $env:CFLAGS
try {
    cargo build --release -p relics-reforged --target-dir target/public
    if ($LASTEXITCODE -ne 0) { throw 'cargo build fallo' }
} finally {
    Remove-Item Env:CARGO_ENCODED_RUSTFLAGS, Env:CFLAGS, Env:CXXFLAGS
}

$meta = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$version = ($meta.packages | Where-Object { $_.name -eq 'relics-reforged' }).version

$dll = Join-Path $root 'target/public/release/relics_reforged.dll'
# La versión pública no puede llevar las herramientas de desarrollo.
if (Select-String -Path $dll -Pattern 'relics_reforged_cmd.txt' -SimpleMatch -Quiet) {
    throw 'La DLL tiene el archivo de comandos: se compilo con la feature dev'
}
# Ni rutas de esta PC ni dependencia del runtime de Visual C++.
foreach ($leak in @($env:USERNAME, $env:USERPROFILE, $root, 'VCRUNTIME140')) {
    $ascii = Select-String -Path $dll -Pattern $leak -SimpleMatch -Quiet -Encoding Default
    $wide = Select-String -Path $dll -Pattern $leak -SimpleMatch -Quiet -Encoding Unicode
    if ($ascii -or $wide) { throw "La DLL contiene '$leak'" }
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
# Compress-Archive de PowerShell 5 guarda las rutas con '\', y otros descompresores (y
# gestores de mods) no las toman como carpetas: se arma a mano con '/'.
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::Open($zip, 'Create')
try {
    foreach ($file in Get-ChildItem $folder -File -Recurse) {
        $entry = 'RelicsReforged/' + $file.FullName.Substring($folder.Length + 1).Replace('\', '/')
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $archive, $file.FullName, $entry, 'Optimal') | Out-Null
    }
} finally {
    $archive.Dispose()
}

Write-Host ''
Write-Host "Listo: $zip"
Get-ChildItem $folder | Format-Table Name, Length -AutoSize
