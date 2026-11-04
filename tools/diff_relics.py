"""Compara dos relics.json por CONTENIDO (item + efectos + maldiciones), porque el ID 0xC08000NN
no es estable entre sesiones. También muestra si el sort_key (orden de adquisición) se conserva."""
import collections, json, sys

sys.stdout.reconfigure(encoding="utf-8")
A = json.load(open(sys.argv[1], encoding="utf-8"))
B = json.load(open(sys.argv[2], encoding="utf-8"))

def key(r):
    return (r["item"], tuple(r["effects"]), tuple(r["curses"]))

ca, cb = collections.Counter(map(key, A)), collections.Counter(map(key, B))
gone, new = ca - cb, cb - ca
by_key_a = {key(r): r for r in A}
by_key_b = {key(r): r for r in B}
print(f"antes={len(A)} ahora={len(B)} | por contenido: faltan={sum(gone.values())} nuevas={sum(new.values())}")
for k, n in gone.items():
    r = by_key_a[k]
    print(f"  VENDIDA: {r['type']:11s} {r['item_name']:30s} x{n} {[x for x in r['effect_names'] if x != '-']}")
for k, n in new.items():
    r = by_key_b[k]
    print(f"  NUEVA:   {r['type']:11s} {r['item_name']:30s} x{n} {[x for x in r['effect_names'] if x != '-']}")

# ¿Qué identificadores se conservan para las reliquias que siguen?
same_id = same_sort = 0
pairs = 0
for k in (ca & cb):
    if ca[k] != 1 or cb[k] != 1:
        continue
    ra, rb = by_key_a[k], by_key_b[k]
    pairs += 1
    same_id += ra["id"] == rb["id"]
    same_sort += ra.get("sort_key") == rb.get("sort_key")
print(f"\nReliquias únicas por contenido presentes en ambos: {pairs} | mismo ID: {same_id} | mismo sort_key: {same_sort}")
