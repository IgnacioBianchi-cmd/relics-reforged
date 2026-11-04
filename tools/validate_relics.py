"""Valida las reliquias de un save contra los params exportados de Smithbox.

Uso: python -I validate_relics.py <relics.json del survey> <carpeta con los CSV>

Prueba dos hipótesis:
  A) estricta: el efecto del slot k está en attachEffectTableId_k (y la maldición en _curse_k)
  B) laxa: el efecto está en alguna de las tablas de la reliquia
Además chequea que no haya dos efectos con el mismo compatibilityId.
"""
import collections, csv, json, sys

sys.stdout.reconfigure(encoding="utf-8")
EMPTY = 0xFFFFFFFF
relics_path, csv_dir = sys.argv[1], sys.argv[2]

def load(name):
    with open(f"{csv_dir}/{name}.csv", encoding="utf-8-sig", newline="") as f:
        return list(csv.DictReader(f))

effects = {int(r["ID"]): r for r in load("AttachEffectParam")}
antique = {int(r["ID"]): r for r in load("EquipParamAntique")}
tables = collections.defaultdict(dict)  # table_id -> {effect_id: (weight, weight_dlc)}
table_rows = load("AttachEffectTableParam")
for r in table_rows:
    tables[int(r["ID"])][int(r["attachEffectId"])] = (int(r["chanceWeight"]), int(r["chanceWeight_dlc"]))

print(f"CSV: AttachEffectParam={len(effects)} filas | EquipParamAntique={len(antique)} filas | "
      f"AttachEffectTableParam={len(table_rows)} entradas en {len(tables)} tablas")

relics = json.load(open(relics_path, encoding="utf-8"))
stats = collections.Counter()
fails = []

def in_table(tid, eff):
    if tid in (-1, 0) or tid not in tables:
        return False
    return eff in tables[tid]

for r in relics:
    if r["type"] == "UniqueRelic":
        continue
    a = antique.get(r["item"])
    if a is None:
        stats["sin fila en EquipParamAntique"] += 1
        fails.append((r, "item no está en EquipParamAntique"))
        continue
    slot_t = [int(a[f"attachEffectTableId_{i}"]) for i in (1, 2, 3)]
    curse_t = [int(a[f"attachEffectTableId_curse{i}"]) for i in (1, 2, 3)]
    if int(a["isDeepRelic"]) != (r["type"] == "DeepRelic"):
        stats["isDeepRelic no coincide con el tipo"] += 1

    strict_ok, lax_ok = True, True
    problems = []
    comp = []
    for k in range(3):
        e, c = r["effects"][k], r["curses"][k]
        if e == EMPTY:
            continue
        if not in_table(slot_t[k], e):
            strict_ok = False
            problems.append(f"slot{k+1} efecto {e} no está en tabla {slot_t[k]}")
        if not any(in_table(t, e) for t in slot_t):
            lax_ok = False
        if c != EMPTY:
            if not in_table(curse_t[k], c):
                strict_ok = False
                problems.append(f"slot{k+1} maldición {c} no está en tabla {curse_t[k]}")
        cid = int(effects[e]["compatibilityId"]) if e in effects else None
        if cid is not None and cid != -1:
            comp.append(cid)
    dup_comp = [x for x, n in collections.Counter(comp).items() if n > 1]
    stats[f"{r['type']}: estricta {'OK' if strict_ok else 'FALLA'}"] += 1
    stats[f"{r['type']}: laxa {'OK' if lax_ok else 'FALLA'}"] += 1
    if dup_comp:
        stats[f"{r['type']}: compatibilityId repetido"] += 1
        problems.append(f"compatibilityId repetido {dup_comp}")
    if problems:
        fails.append((r, "; ".join(problems)))

print("\nResultados:")
for k, v in sorted(stats.items()):
    print(f"  {k}: {v}")
print(f"\nReliquias con algún problema: {len(fails)}")
for r, why in fails[:15]:
    print(f"  - {r['item_name']} (item {r['item']}): {why}")

# Estructura de las tablas por tamaño, para el doc
print("\nTablas usadas por tamaño/tipo (reliquias aleatorias del save):")
seen = collections.Counter()
for r in relics:
    if r["type"] == "UniqueRelic" or r["item"] not in antique:
        continue
    a = antique[r["item"]]
    key = (r["type"], r["item_name"].split()[0] if r["type"] == "Relic" else " ".join(r["item_name"].split()[:2]),
           tuple(int(a[f"attachEffectTableId_{i}"]) for i in (1, 2, 3)),
           tuple(int(a[f"attachEffectTableId_curse{i}"]) for i in (1, 2, 3)))
    seen[key] += 1
for k, v in sorted(seen.items()):
    print(f"  {k}: {v}")
