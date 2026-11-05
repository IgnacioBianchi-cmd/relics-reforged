"""Dado un puntero a una entrada de vtable en el volcado (RVA), encuentra el comienzo de la vtable
y el nombre de la clase por RTTI (MSVC x64: el qword anterior apunta al CompleteObjectLocator,
que tiene en +0x0C la RVA del TypeDescriptor, cuyo nombre empieza en +0x10).

Uso: vtable_class.py <volcado> <base del volcado hex> <rva de la entrada>
"""
import struct
import sys

sys.stdout.reconfigure(encoding="utf-8")
dump = open(sys.argv[1], "rb").read()
base = int(sys.argv[2], 16)
entry = int(sys.argv[3], 16)
text_end = 0x2BB5000  # fin de .text (las entradas de una vtable apuntan a código)


def q(o):
    return struct.unpack_from("<Q", dump, o)[0]


def is_code(v):
    return base + 0x1000 <= v < base + text_end


start = entry
while is_code(q(start - 8)):
    start -= 8
col = q(start - 8) - base
print(f"vtable en nr+{start:X} (entrada {(entry - start) // 8}), COL en nr+{col:X}")
type_rva = struct.unpack_from("<I", dump, col + 0x0C)[0]
name_end = dump.index(b"\0", type_rva + 0x10)
print("clase:", dump[type_rva + 0x10:name_end].decode(errors="replace"))
count = 0
while is_code(q(start + count * 8)):
    count += 1
print(f"{count} entradas")
