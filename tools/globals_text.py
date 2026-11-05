"""Muestra como texto UTF-16 las variables globales que cambian entre dos fotos de `dumpdata`.

Uso: globals_text.py A.bin B.bin  (por ejemplo, menú cerrado y menú abierto)
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


def text_at(buf, off):
    out = []
    for i in range(off, min(len(buf) - 1, off + 64), 2):
        c = struct.unpack_from("<H", buf, i)[0]
        if c == 0:
            break
        if not (32 <= c < 127):
            return ""
        out.append(chr(c))
    return "".join(out)


a, b = load(sys.argv[1]), load(sys.argv[2])
shown = set()
for rva, buf_a in a.items():
    buf_b = b[rva]
    for off in range(0, len(buf_a) - 16, 16):
        if buf_a[off:off + 16] == buf_b[off:off + 16]:
            continue
        ta, tb = text_at(buf_a, off), text_at(buf_b, off)
        if (len(tb) >= 3 or len(ta) >= 3) and ta != tb and (rva + off) // 64 not in shown:
            shown.add((rva + off) // 64)
            print(f"nr+{rva + off:X}: A '{ta}'  |  B '{tb}'")
