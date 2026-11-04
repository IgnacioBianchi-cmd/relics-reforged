"""Ubica en un save nuevo el campo que estaba en <offset> del save viejo, usando los bytes vecinos.

Uso: python -I locate_by_context.py <antes.bin> <despues.bin> <offset_hex>
"""
import struct, sys

sys.stdout.reconfigure(encoding="utf-8")
a = open(sys.argv[1], "rb").read()
b = open(sys.argv[2], "rb").read()
off = int(sys.argv[3], 16)

def dump(data, start, n=64):
    for row in range(start, start + n, 16):
        chunk = data[row:row + 16]
        u32 = [struct.unpack_from("<I", data, row + i)[0] for i in range(0, 16, 4)]
        print(f"   0x{row:06X}: {chunk.hex(' ')}   u32={u32}")

print("Backup alrededor del murk:")
dump(a, off - 32)

for before, after in ((32, 0), (16, 16), (0, 32), (24, 8), (8, 24), (64, 0), (0, 64)):
    ctx_pre = a[off - before:off]
    ctx_post = a[off + 4:off + 4 + after]
    if before:
        i = b.find(ctx_pre)
        hits = []
        while i != -1 and len(hits) < 5:
            cand = i + before
            if not after or b[cand + 4:cand + 4 + after] == ctx_post:
                hits.append(cand)
            i = b.find(ctx_pre, i + 1)
    else:
        i = b.find(ctx_post)
        hits = []
        while i != -1 and len(hits) < 5:
            hits.append(i - 4)
            i = b.find(ctx_post, i + 1)
    if hits:
        print(f"\nContexto {before}B antes + {after}B después -> {len(hits)} coincidencia(s): {[hex(h) for h in hits]}")
        for h in hits[:2]:
            print(f"   valor en 0x{h:06X} ahora: {struct.unpack_from('<I', b, h)[0]}  (desplazamiento {h - off:+d})")
            dump(b, h - 32)
        break
else:
    print("No se encontró el contexto.")
