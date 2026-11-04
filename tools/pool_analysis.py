"""Clasifica reliquias en aleatorias vs fijas y compara los pools por slot."""
import collections, csv, json, sys

sys.stdout.reconfigure(encoding="utf-8")
EMPTY = 0xFFFFFFFF
relics_path, csv_dir = sys.argv[1], sys.argv[2]

def load(name):
    with open(f"{csv_dir}/{name}.csv", encoding="utf-8-sig", newline="") as f:
        return list(csv.DictReader(f))

antique = {int(r["ID"]): r for r in load("EquipParamAntique")}
effects = {int(r["ID"]): r for r in load("AttachEffectParam")}
tables = collections.defaultdict(dict)
for r in load("AttachEffectTableParam"):
    w, wd = int(r["chanceWeight"]), int(r["chanceWeight_dlc"])
    tables[int(r["ID"])][int(r["attachEffectId"])] = (w, wd)

def is_random(tid):
    return tid not in (-1, 0) and len([e for e in tables.get(tid, {}) if e != -1]) > 1

relics = json.load(open(relics_path, encoding="utf-8"))
kinds = collections.Counter()
for r in relics:
    a = antique.get(r["item"])
    if not a:
        continue
    t = [int(a[f"attachEffectTableId_{i}"]) for i in (1, 2, 3)]
    rnd = [is_random(x) for x in t if x != -1]
    kind = "aleatoria" if all(rnd) else ("fija" if not any(rnd) else "mixta")
    kinds[(r["type"], kind, "vendible" if a["isSalable"] == "1" else "no vendible")] += 1
print("Clasificación de las reliquias del save:")
for k, v in sorted(kinds.items()):
    print(f"  {k}: {v}")

def pool(tid):
    return {e for e, (w, wd) in tables[tid].items() if e != -1 and (w > 0 or wd > 0)}

print("\nPools por slot (efectos con peso > 0):")
for name, tids in [("Normal 300/200/100", (300, 200, 100)), ("Normal 310/210/110", (310, 210, 110)),
                   ("Normal 1001002/1/0", (1001002, 1001001, 1001000)),
                   ("Deep 2000000 / 2200000", (2000000, 2200000)), ("Maldición Deep 3000000", (3000000,))]:
    sets = [pool(t) for t in tids]
    print(f"  {name}: tamaños {[len(s) for s in sets]}")
    if len(sets) >= 2:
        for i in range(len(sets)):
            for j in range(i + 1, len(sets)):
                inter = len(sets[i] & sets[j])
                print(f"     tabla {tids[i]} ∩ {tids[j]} = {inter}  | solo en {tids[i]}: {len(sets[i]-sets[j])}  | solo en {tids[j]}: {len(sets[j]-sets[i])}")

# ¿Qué distingue a las tablas del slot 1 de una Grande?
only1 = pool(300) - pool(200) - pool(100)
print(f"\nEfectos que SOLO pueden salir en el slot 1 de una Grand (300): {len(only1)}. Ejemplos:")
for e in sorted(only1)[:12]:
    print("   ", e, effects.get(e, {}).get("Name", "?"))
by_cat = collections.Counter(int(effects[e]["overrideEffectId"]) for e in only1 if e in effects)
print("   overrideEffectId de esos efectos:", dict(by_cat))

curse = pool(3000000)
print(f"\nMaldiciones posibles (tabla 3000000): {len(curse)}; todas isDebuff=1:",
      all(effects.get(e, {}).get("isDebuff") == "1" for e in curse))
