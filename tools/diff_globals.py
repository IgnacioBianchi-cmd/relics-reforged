"""Compara fotos de las variables globales de nightreign.exe (comando `dumpdata` de la DLL).

Uso: diff_globals.py cerrado1.bin abierto.bin cerrado2.bin [abierto2.bin cerrado3.bin ...]

Las fotos alternan estado A (por ejemplo, menú cerrado) y estado B (menú abierto). Se listan
los u32/u64 alineados que valen lo mismo en todas las fotos A, lo mismo en todas las B, y
distinto entre A y B: candidatos a "marca" de ese estado.
"""
import struct
import sys

sys.stdout.reconfigure(encoding="utf-8")


def load(path):
    data = open(path, "rb").read()
    sections, pos = {}, 0
    while pos < len(data):
        rva, size = struct.unpack_from("<II", data, pos)
        sections[rva] = data[pos + 8:pos + 8 + size]
        pos += 8 + size
    return sections


snaps = [load(p) for p in sys.argv[1:]]
a_snaps, b_snaps = snaps[0::2], snaps[1::2]
found = []
for rva, first in snaps[0].items():
    n = len(first)
    for off in range(0, n - 7, 4):
        a_vals = {s[rva][off:off + 4] for s in a_snaps}
        b_vals = {s[rva][off:off + 4] for s in b_snaps}
        if len(a_vals) == 1 and len(b_vals) == 1 and a_vals != b_vals:
            a = struct.unpack("<I", next(iter(a_vals)))[0]
            b = struct.unpack("<I", next(iter(b_vals)))[0]
            found.append((rva + off, a, b))
print(f"{len(found)} candidatos")
for addr, a, b in found[:200]:
    print(f"  nr+{addr:X}: cerrado {a:08X}  abierto {b:08X}")
