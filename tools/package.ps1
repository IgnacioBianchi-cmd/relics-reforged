# Arma la versión pública de Relics Reforged para publicar (Nexus Mods).
#
# Compila la DLL sin la feature "dev" en target/public (así no pisa la de desarrollo de
# target/release, que usa me3/relics-reforged-dev.me3) y deja en target/dist:
#   RelicsReforged-v<ver>.zip             archivo principal: RelicsReforged.dll y la carpeta
#                                         RelicsReforged\ (README y licencia; ahí la DLL crea el
#                                         .ini y el log), en la raíz del zip, como los demás mods
#                                         DLL: se arrastra a ME3 Manager o se descomprime en la
#                                         carpeta de mods de me3 o de Elden Mod Loader. Sin .me3:
#                                         ME3 Manager importa como perfil cualquier zip que traiga uno.
#   RelicsReforged-Standalone-v<ver>.zip  archivo opcional: lo mismo dentro de una carpeta, con el
#                                         perfil RelicsReforged.me3 (sin conexión, save aparte)
#   main\ y RelicsReforged-Standalone\    las mismas carpetas, sin comprimir, para probar
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
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $dist | Out-Null

# La DLL y su carpeta (README y licencia) en una carpeta: la del archivo principal o la del
# standalone.
function Add-ModFiles($dest) {
    $files = Join-Path $dest 'RelicsReforged'
    New-Item -ItemType Directory -Force $files | Out-Null
    Copy-Item $dll (Join-Path $dest 'RelicsReforged.dll')
    Copy-Item (Join-Path $root 'dist/README.txt') $files
    Copy-Item (Join-Path $root 'LICENSE') $files
}

# Arma el zip a mano: Compress-Archive de PowerShell 5 guarda las rutas con '\', y otros
# descompresores (y gestores de mods) no las toman como carpetas. $prefix va delante de cada
# ruta ('' = el contenido de $folder en la raíz del zip).
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
function New-ModZip($folder, $zip, $prefix) {
    $archive = [System.IO.Compression.ZipFile]::Open($zip, 'Create')
    try {
        foreach ($file in Get-ChildItem $folder -File -Recurse) {
            $entry = $prefix + $file.FullName.Substring($folder.Length + 1).Replace('\', '/')
            [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                $archive, $file.FullName, $entry, 'Optimal') | Out-Null
        }
    } finally {
        $archive.Dispose()
    }
}

$main = Join-Path $dist 'main'
Add-ModFiles $main
$mainZip = Join-Path $dist "RelicsReforged-v$version.zip"
New-ModZip $main $mainZip ''

$standalone = Join-Path $dist 'RelicsReforged-Standalone'
Add-ModFiles $standalone
Copy-Item (Join-Path $root 'dist/standalone/RelicsReforged.me3') $standalone
$standaloneZip = Join-Path $dist "RelicsReforged-Standalone-v$version.zip"
New-ModZip $standalone $standaloneZip 'RelicsReforged-Standalone/'

Write-Host ''
Write-Host "Listo: $mainZip"
Write-Host "       $standaloneZip"
