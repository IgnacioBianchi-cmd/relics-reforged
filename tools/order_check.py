import json, sys, collections
sys.stdout.reconfigure(encoding="utf-8")
relics = json.load(open(sys.argv[1], encoding="utf-8"))
EMPTY = 0xFFFFFFFF

asc = desc = other = 0
for r in relics:
    used = [e for e in r["effects"] if e != EMPTY]
    if len(used) < 2:
        continue
    if used == sorted(used): asc += 1
    elif used == sorted(used, reverse=True): desc += 1
    else: other += 1
print(f"Orden por ID: asc={asc} desc={desc} otro={other}")

# ¿Hay patrón por "familia" de ID (millones / cientos de miles)?
by_slot = collections.defaultdict(collections.Counter)
for r in relics:
    if r["type"] != "Relic":
        continue
    for i, e in enumerate(r["effects"]):
        if e != EMPTY:
            by_slot[i][e // 100000] += 1
for i in sorted(by_slot):
    print(f"slot {i}: familias (ID//100000) ->", dict(sorted(by_slot[i].items())))

print("\nEjemplos de reliquias normales de 3 efectos (IDs y nombres):")
for r in [x for x in relics if x["type"] == "Relic" and sum(e != EMPTY for e in x["effects"]) == 3][:8]:
    print(" ", r["item_name"])
    for e, n in zip(r["effects"], r["effect_names"]):
        if e != EMPTY:
            print(f"     {e:>9}  {n}")

print("\nReliquias con bloque medio distinto:")
for r in relics:
    if "0000feff" in r["mid_hex"]:
        print(" ", r["id"], r["type"], r["item_name"], r["effect_names"][:3], r["mid_hex"])
