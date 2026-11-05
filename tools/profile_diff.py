"""Compara dos perfiles de la DLL (comando `profile`) del hilo principal: funciones que aparecen
en la pila (una vez por muestra) en un estado y no en el otro.

Uso: profile_diff.py <volcado nightreign> <base hex> <perfil A> <perfil B>
Las funciones se agrupan por .pdata del volcado (`dumpimage`).
"""
import bisect
import struct
import sys
from collections import Counter

sys.stdout.reconfigure(encoding="utf-8")
dump_path, base_s, prof_a, prof_b = sys.argv[1:5]
ALL_THREADS = sys.argv[5] if len(sys.argv) > 5 else "0"  # "1" = todos los hilos
base = int(base_s, 16)
dump = open(dump_path, "rb").read()
nt = struct.unpack_from("<I", dump, 0x3C)[0]
opt = nt + 24
pdata_rva, pdata_len = struct.unpack_from("<II", dump, opt + 112 + 3 * 8)
funcs = sorted(
    struct.unpack_from("<II", dump, pdata_rva + i * 12)
    for i in range(pdata_len // 12)
)
starts = [f[0] for f in funcs]
ENTRY_RVA = 0x26916BC


def func_of(addr):
    rva = addr - base
    if not 0 <= rva < len(dump):
        return None
    i = bisect.bisect_right(starts, rva) - 1
    if i >= 0 and funcs[i][0] <= rva < funcs[i][1]:
        return funcs[i][0]
    return None


def load(path):
    main = None
    samples = []
    for line in open(path, encoding="utf-8"):
        parts = line.split()
        if parts[0] == "T" and int(parts[2], 16) == base + ENTRY_RVA:
            main = parts[1]
        elif parts[0] == "S":
            samples.append(parts)
    counts = Counter()
    n = 0
    for p in samples:
        if ALL_THREADS == "0" and p[1] != main:
            continue
        n += 1
        fs = {func_of(int(x, 16)) for x in p[2:]}
        fs.discard(None)
        counts.update(fs)
    return counts, n


a, na = load(prof_a)
b, nb = load(prof_b)
print(f"{'todos los hilos' if ALL_THREADS == '1' else 'hilo principal'}: {na} muestras en A, {nb} en B")
rows = []
for f in set(a) | set(b):
    fa, fb = a[f] / max(na, 1), b[f] / max(nb, 1)
    rows.append((fb - fa, f, fa, fb))
rows.sort(reverse=True)
print("solo (o mucho más) con el estado B:")
for d, f, fa, fb in rows[:40]:
    print(f"  nr+{f:X}: A {fa:6.1%}  B {fb:6.1%}")
print("solo con el estado B (0 en A):")
only_b = sorted(((b[f], f) for f in b if a[f] == 0), reverse=True)
for c, f in only_b[:40]:
    print(f"  nr+{f:X}: {c} muestras")
