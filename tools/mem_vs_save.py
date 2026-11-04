"""Compara distancias en memoria (direcciones de Cheat Engine) con offsets del save."""
import json, sys

sys.stdout.reconfigure(encoding="utf-8")
rs = json.load(open(sys.argv[1], encoding="utf-8"))
r = [x for x in rs if x["item"] == 0x1EB0B9 and x["effects"][0] == 0x5BA360][0]
murk = 0x10E18
print(f"save: reliquia 0x{r['offset']:X} | murk 0x{murk:X} | distancia 0x{murk - r['offset']:X}")
copies = {
    "A": (0x7FF3B17C8208, 0x7FF3B2F3BF28),
    "B": (0x7FF3C9FA61D0, 0x7FF3C9FB48F0),
    "C": (0x7FF3CA206230, 0x7FF3CA214950),
}
for name, (rel, mk) in copies.items():
    print(f"{name}: distancia reliquia->murk 0x{mk - rel:X} | inicio de USERDATA_00 si fuera imagen: 0x{rel - r['offset']:X}")
