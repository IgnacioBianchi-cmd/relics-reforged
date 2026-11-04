param(
    [Parameter(Mandatory)] [string]$SavePath,
    [Parameter(Mandatory)] [string]$OutDir
)
# Desencripta las entradas BND4 de un .sl2 de Nightreign (AES-128-CBC, IV en los primeros 16 bytes).
# Escribe cada entrada como USERDATA_XX.bin (sin los 4 bytes iniciales de checksum/tamaño).
$key = [byte[]](0x18,0xf6,0x32,0x66,0x05,0xbd,0x17,0x8a,0x55,0x24,0x52,0x3a,0xc0,0xa0,0xc6,0x09)
$raw = [IO.File]::ReadAllBytes($SavePath)
if ([Text.Encoding]::ASCII.GetString($raw, 0, 4) -ne 'BND4') { throw 'No es un BND4' }
$count = [BitConverter]::ToInt32($raw, 12)
New-Item -ItemType Directory -Force $OutDir | Out-Null
$aes = [Security.Cryptography.Aes]::Create()
$aes.Mode = 'CBC'; $aes.Padding = 'None'; $aes.Key = $key
for ($i = 0; $i -lt $count; $i++) {
    $pos = 64 + 32 * $i
    $size = [BitConverter]::ToInt32($raw, $pos + 8)
    $off = [BitConverter]::ToInt32($raw, $pos + 16)
    $iv = New-Object byte[] 16
    [Array]::Copy($raw, $off, $iv, 0, 16)
    $aes.IV = $iv
    $dec = $aes.CreateDecryptor().TransformFinalBlock($raw, $off + 16, $size - 16)
    $clean = New-Object byte[] ($dec.Length - 4)
    [Array]::Copy($dec, 4, $clean, 0, $clean.Length)
    [IO.File]::WriteAllBytes((Join-Path $OutDir ('USERDATA_{0:D2}.bin' -f $i)), $clean)
    Write-Output ('entry {0}: {1} bytes' -f $i, $clean.Length)
}
