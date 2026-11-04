"""Busca el offset del murk en USERDATA_00 comparando dos saves desencriptados.

Uso: python -I diff_murk.py <antes.bin> <despues.bin> <murk_antes> <murk_despues_1> [<murk_despues_2> ...]
Lista los offsets donde 'antes' tiene murk_antes y 'despues' tiene alguno de los candidatos.
"""
import struct, sys

sys.stdout.reconfigure(encoding="utf-8")
a = open(sys.argv[1], "rb").read()
b = open(sys.argv[2], "rb").read()
before = int(sys.argv[3])
cands = {int(x) for x in sys.argv[4:]}
print(f"tamaños: {len(a)} / {len(b)} | bytes distintos: {sum(x != y for x, y in zip(a, b))}")
hits = []
for off in range(0, min(len(a), len(b)) - 4, 4):
    va = struct.unpack_from("<I", a, off)[0]
    if va == before:
        vb = struct.unpack_from("<I", b, off)[0]
        hits.append((off, vb, vb in cands))
print(f"offsets con {before} en el backup: {len(hits)}")
for off, vb, ok in hits:
    print(f"  0x{off:06X}: después = {vb}{'   <-- CANDIDATO' if ok else ''}")
