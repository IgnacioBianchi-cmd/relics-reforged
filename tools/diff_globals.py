"""Compara fotos de las variables globales de nightreign.exe (comando `dumpdata` de la DLL).

Uso: diff_globals.py A1.bin A2.bin ... -- B1.bin B2.bin ...

Lista los u32 alineados que valen lo mismo en todas las fotos del grupo A, lo mismo en todas las
del grupo B, y distinto entre A y B: candidatos a "marca" del estado B. Conviene tomar varias
fotos seguidas de cada estado: así lo que cambia frame a frame queda afuera.
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


args = sys.argv[1:]
if "--" not in args:
    sys.exit("uso: diff_globals.py A1 A2 ... -- B1 B2 ...")
sep = args.index("--")
a_snaps = [load(p) for p in args[:sep]]
b_snaps = [load(p) for p in args[sep + 1:]]
found = []
for rva, first in a_snaps[0].items():
    for off in range(0, len(first) - 3, 4):
        a_vals = {s[rva][off:off + 4] for s in a_snaps}
        if len(a_vals) != 1:
            continue
        b_vals = {s[rva][off:off + 4] for s in b_snaps}
        if len(b_vals) == 1 and a_vals != b_vals:
            a = struct.unpack("<I", next(iter(a_vals)))[0]
            b = struct.unpack("<I", next(iter(b_vals)))[0]
            found.append((rva + off, a, b))
print(f"{len(found)} candidatos ({len(a_snaps)} fotos A, {len(b_snaps)} fotos B)")
for addr, a, b in found[:200]:
    print(f"  nr+{addr:X}: A {a:08X}  B {b:08X}")
