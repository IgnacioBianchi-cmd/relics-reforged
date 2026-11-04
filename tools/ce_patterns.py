"""Genera patrones de búsqueda (Array of Byte) para Cheat Engine a partir de relics.json.

Elige una reliquia normal aleatoria de 3 efectos y una Deep con maldición, y escribe
para cada una el AOB de la cabecera del registro (16 bytes) y el de sus efectos.
"""
import json, struct, sys

sys.stdout.reconfigure(encoding="utf-8")
EMPTY = 0xFFFFFFFF
relics = json.load(open(sys.argv[1], encoding="utf-8"))

def aob(*u32):
    return " ".join(f"{b:02X}" for v in u32 for b in struct.pack("<I", v))

def pick(pred):
    for r in relics:
        if pred(r):
            return r

normal = pick(lambda r: r["type"] == "Relic" and r["item_name"].startswith("Grand")
              and sum(e != EMPTY for e in r["effects"]) == 3 and r["item"] in range(1000, 2000))
deep = pick(lambda r: r["type"] == "DeepRelic" and any(c != EMPTY for c in r["curses"]))

for label, r in (("Normal", normal), ("Deep", deep)):
    if not r:
        continue
    item = r["item"] | 0x80000000
    print(f"== {label}: {r['item_name']}  (ID único {r['id']} = 0x{r['id']:08X})")
    for e, n, c, cn in zip(r["effects"], r["effect_names"], r["curses"], r["curse_names"]):
        if e != EMPTY:
            print(f"     · {n}" + (f"   [maldición: {cn}]" if c != EMPTY else ""))
    print(f"   AOB cabecera : {aob(r['id'], item, item, EMPTY)}")
    print(f"   AOB efectos  : {aob(*r['effects'])}")
    print(f"   ID único como 4 bytes (con signo, para 'Exact Value'): {struct.unpack('<i', struct.pack('<I', r['id']))[0]}")
