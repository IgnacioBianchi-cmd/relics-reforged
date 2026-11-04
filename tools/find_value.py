"""Busca valores u32 (alineados o no) en todas las entradas desencriptadas de dos saves.

Uso: python -I find_value.py <dir_antes> <dir_despues> <valor_antes> <cand1> [<cand2> ...]
"""
import os, struct, sys

sys.stdout.reconfigure(encoding="utf-8")
d_before, d_after, v_before = sys.argv[1], sys.argv[2], int(sys.argv[3])
cands = [int(x) for x in sys.argv[4:]]

def find_all(data, value):
    pat = struct.pack("<I", value)
    out, i = [], data.find(pat)
    while i != -1:
        out.append(i)
        i = data.find(pat, i + 1)
    return out

for name in sorted(os.listdir(d_before)):
    a = open(os.path.join(d_before, name), "rb").read()
    b = open(os.path.join(d_after, name), "rb").read() if os.path.exists(os.path.join(d_after, name)) else b""
    hb = find_all(a, v_before)
    ha = {c: find_all(b, c) for c in cands}
    ha = {c: v for c, v in ha.items() if v}
    if hb or ha:
        print(f"== {name}")
        if hb:
            print(f"   antes  {v_before}: {[hex(x) for x in hb[:10]]}")
        for c, offs in ha.items():
            print(f"   ahora  {c}: {[hex(x) for x in offs[:10]]}")
