"""Relevamiento de reliquias: lee USERDATA_00.bin (ya desencriptado) y resume la estructura.

Layout del registro de reliquia (80 bytes), tomado del parser de ERN_RelicForge:
  +0   u32 relic_id (byte 3 = 0xC0)
  +4   u32 item_id | 0x80000000
  +8   u32 item_id | 0x80000000
  +12  u32 0xFFFFFFFF
  +16  u32 x4 efectos
  +32  ?? (24 bytes, desconocido)
  +56  u32 x4 maldiciones (emparejadas por índice con los efectos)
  +72  ?? (8 bytes, desconocido)
"""
import collections, json, struct, sys

sys.stdout.reconfigure(encoding="utf-8")
EMPTY = 0xFFFFFFFF

entry_path, res_dir, out_path = sys.argv[1], sys.argv[2], sys.argv[3]
data = open(entry_path, "rb").read()
items = {int(k): v for k, v in json.load(open(f"{res_dir}/items_data.json", encoding="utf-8"))["items"].items()}
effects = {int(k): v for k, v in json.load(open(f"{res_dir}/effects_data.json", encoding="utf-8"))["effects"].items()}

def size_of(name):
    for word, n in (("Delicate", 1), ("Polished", 2), ("Grand", 3)):
        if word in name:
            return n
    return None

relics, seen = [], set()
pos, limit = 0, len(data) - 80
while pos < limit:
    if data[pos + 3] == 0xC0 and data[pos + 7] == 0x80 and data[pos + 11] == 0x80 \
            and struct.unpack_from("<I", data, pos + 12)[0] == EMPTY:
        rid = struct.unpack_from("<I", data, pos)[0]
        item_a = struct.unpack_from("<I", data, pos + 4)[0] & 0x7FFFFFFF
        item_b = struct.unpack_from("<I", data, pos + 8)[0] & 0x7FFFFFFF
        effs = struct.unpack_from("<4I", data, pos + 16)
        curses = struct.unpack_from("<4I", data, pos + 56)
        if item_a in items and rid not in seen and any(e != EMPTY for e in effs):
            seen.add(rid)
            relics.append({
                "offset": pos, "id": rid, "item": item_a, "item_b_same": item_a == item_b,
                "effects": list(effs), "curses": list(curses),
                "mid_hex": data[pos + 32:pos + 56].hex(" ", 4),
                "tail_hex": data[pos + 72:pos + 80].hex(" ", 4),
            })
            pos += 80
            continue
    pos += 1

def ename(e):
    return effects.get(e, {}).get("name_en", f"?{e}") if e != EMPTY else "-"

by_type = collections.Counter(items[r["item"]]["type"] for r in relics)
print("Reliquias encontradas:", len(relics), dict(by_type))
print("item_id duplicado igual en ambos campos:", all(r["item_b_same"] for r in relics))
print("Offsets: primera", relics[0]["offset"] if relics else None, "última", relics[-1]["offset"] if relics else None)

shape = collections.Counter()
curse_without_effect = 0
fourth_slot_used = 0
curse_on_normal = 0
order_issues = 0
for r in relics:
    it = items[r["item"]]
    n_eff = sum(e != EMPTY for e in r["effects"])
    n_cur = sum(c != EMPTY for c in r["curses"])
    pattern = "".join(("E" if e != EMPTY else "_") + ("c" if c != EMPTY else " ") for e, c in zip(r["effects"], r["curses"]))
    shape[(it["type"], size_of(it["name_en"]), n_eff, n_cur, pattern)] += 1
    curse_without_effect += sum(1 for e, c in zip(r["effects"], r["curses"]) if e == EMPTY and c != EMPTY)
    fourth_slot_used += r["effects"][3] != EMPTY
    if it["type"] != "DeepRelic" and n_cur:
        curse_on_normal += 1
    used = [e for e in r["effects"] if e != EMPTY]
    if used != sorted(used):
        order_issues += 1

print("\nForma (tipo, tamaño por nombre, #efectos, #maldiciones, patrón por slot) -> cantidad")
for k, v in sorted(shape.items(), key=lambda kv: (str(kv[0][0]), str(kv[0][1]), kv[0][2], kv[0][3])):
    print(f"  {k}: {v}")
print("\nMaldición en slot sin efecto:", curse_without_effect)
print("4º slot de efecto usado:", fourth_slot_used)
print("Reliquias no-Deep con maldición:", curse_on_normal)
print("Reliquias con efectos NO ordenados ascendentemente por ID:", order_issues)

mids = collections.Counter(r["mid_hex"] for r in relics)
tails = collections.Counter(r["tail_hex"] for r in relics)
print("\nBloque +32..+56, valores más comunes:")
for k, v in mids.most_common(5):
    print(f"  {v:5d} x  {k}")
print("Bloque +72..+80, valores más comunes:")
for k, v in tails.most_common(5):
    print(f"  {v:5d} x  {k}")

deep_examples = [r for r in relics if items[r["item"]]["type"] == "DeepRelic"][:4]
print("\nEjemplos Deep:")
for r in deep_examples:
    print(" ", items[r["item"]]["name_en"])
    for e, c in zip(r["effects"], r["curses"]):
        if e != EMPTY or c != EMPTY:
            print(f"     {ename(e)}  ||  maldición: {ename(c)}")

curse_ids = {c for r in relics for c in r["curses"] if c != EMPTY}
print("\nIDs de maldición distintos:", len(curse_ids), "rango", min(curse_ids, default=None), "-", max(curse_ids, default=None))
eff_ids = {e for r in relics for e in r["effects"] if e != EMPTY}
print("IDs de efecto distintos:", len(eff_ids), "| desconocidos:", sorted(e for e in eff_ids | curse_ids if e not in effects)[:20])

json.dump([{**r, "item_name": items[r["item"]]["name_en"], "type": items[r["item"]]["type"],
            "effect_names": [ename(e) for e in r["effects"]], "curse_names": [ename(c) for c in r["curses"]]}
           for r in relics], open(out_path, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print("\nDetalle escrito en", out_path)
