"""Busca en las mediciones F8 (relics_reforged_f8_N.txt) valores que distingan estados de
Relic Rites: constantes dentro de cada estado y distintos entre estados.

Uso: python tools/f8_states.py <carpeta> S,G,S,C,S,G,S,C,S
  (una letra por medición, en orden: S slots, G grilla, C eligiendo para un slot)
"""
import sys
from collections import defaultdict
from pathlib import Path


def load(path):
    words, noise, ventana = {}, set(), None
    for line in path.read_text().splitlines():
        if line.startswith("ventana"):
            ventana = line.split()[1]
            continue
        if line.startswith("llamadas"):
            continue
        parts = line.split()
        key = (parts[0], parts[1])
        words[key] = int(parts[2], 16)
        if len(parts) > 3:
            noise.add(key)
    return ventana, words, noise


folder, labels = Path(sys.argv[1]), sys.argv[2].split(",")
runs = [load(folder / f"relics_reforged_f8_{i + 1}.txt") for i in range(len(labels))]
noise = set().union(*(r[2] for r in runs))
common = set.intersection(*(set(r[1]) for r in runs)) - noise


def parent_ptr(run, parent):
    """qword de la ventana en el offset `parent` (hex)."""
    if parent == "-":
        return None
    off = int(parent, 16)
    lo = run[1].get(("-", f"{off:X}"))
    hi = run[1].get(("-", f"{off + 4:X}"))
    return None if lo is None or hi is None else (hi << 32) | lo


results = []
for key in common:
    by_state = defaultdict(set)
    for (_, words, _), lab in zip(runs, labels):
        by_state[lab].add(words[key])
    if any(len(v) != 1 for v in by_state.values()):
        continue
    values = {lab: next(iter(v)) for lab, v in by_state.items()}
    if len(set(values.values())) < len(values):
        continue
    stable = len({parent_ptr(r, key[0]) for r in runs}) == 1
    results.append((key[0] != "-", not stable, key, values))

results.sort(key=lambda r: (r[0], r[1], max(r[3].values()), r[2]))
print(f"{len(results)} valores distinguen los {len(set(labels))} estados")
for child, unstable, key, values in results[:80]:
    where = f"dlg+{key[1]}" if key[0] == "-" else f"[dlg+{key[0]}]+{key[1]}"
    flag = " (puntero padre cambia)" if unstable else ""
    print(f"{where:24} " + " ".join(f"{k}={v:X}" for k, v in sorted(values.items())) + flag)
