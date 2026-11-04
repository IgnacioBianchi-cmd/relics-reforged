"""Busca qué campos de AttachEffectParam explican el orden de los efectos dentro de una reliquia.

Prueba claves simples y compuestas (primaria, secundaria), ignorando campos constantes.
"""
import collections, csv, itertools, json, sys

sys.stdout.reconfigure(encoding="utf-8")
EMPTY = 0xFFFFFFFF
relics_path, csv_dir = sys.argv[1], sys.argv[2]
with open(f"{csv_dir}/AttachEffectParam.csv", encoding="utf-8-sig", newline="") as f:
    effects = {int(r["ID"]): r for r in csv.DictReader(f)}

relics = [r for r in json.load(open(relics_path, encoding="utf-8"))
          if sum(e != EMPTY for e in r["effects"]) >= 2]
used = sorted({e for r in relics for e in r["effects"] if e != EMPTY})

def val(field, e):
    if field == "ID":
        return e
    try:
        return int(effects[e][field])
    except (KeyError, ValueError):
        return 0

fields = ["ID"] + [k for k in next(iter(effects.values())).keys() if k not in ("ID", "Name")]
# solo campos que varían entre los efectos usados
fields = [f for f in fields if len({val(f, e) for e in used}) > 1]
print("Campos que varían:", fields)

def score(key):
    ok_asc = ok_desc = strict = 0
    for r in relics:
        vals = [key(e) for e in r["effects"] if e != EMPTY]
        if vals == sorted(vals):
            ok_asc += 1
        if vals == sorted(vals, reverse=True):
            ok_desc += 1
        if len(set(vals)) == len(vals):
            strict += 1
    return ok_asc, ok_desc, strict

rows = []
for f in fields:
    a, d, s = score(lambda e, f=f: val(f, e))
    rows.append((max(a, d), s, f"{f}", a, d))
for f1, f2 in itertools.permutations(fields, 2):
    for sign in (1, -1):
        a, d, s = score(lambda e, f1=f1, f2=f2, sign=sign: (val(f1, e), sign * val(f2, e)))
        rows.append((max(a, d), s, f"({f1}, {'+' if sign > 0 else '-'}{f2})", a, d))
rows.sort(reverse=True)
print(f"\nTotal reliquias con 2+ efectos: {len(relics)}")
for best, s, name, a, d in rows[:15]:
    print(f"  {name:55s} asc={a:3d} desc={d:3d} sin_empates={s:3d}")

print("\nValores de overrideEffectId entre los efectos usados:",
      dict(collections.Counter(val('overrideEffectId', e) for e in used).most_common(15)))

best = rows[0][2]
print(f"\nContraejemplos de {best}:")
if best.startswith("("):
    f1, rest = best[1:-1].split(", ")
    sign, f2 = (1 if rest[0] == "+" else -1), rest[1:]
    key = lambda e: (val(f1, e), sign * val(f2, e))
else:
    key = lambda e: val(best, e)
n = 0
for r in relics:
    vals = [key(e) for e in r["effects"] if e != EMPTY]
    if vals != sorted(vals):
        names = [x for x in r["effect_names"] if x != "-"]
        print(f"  {r['type']:10s} {r['item_name']}: {list(zip(names, vals))}")
        n += 1
        if n >= 10:
            break
