"""Recorta íconos del menú de Nightreign a PNG para revisarlos (prueba antes de pasarlo a Rust).

Entradas (generadas con tools/nr_archive.py ... get):
  data/game/01_common_h.tpf        TPF descomprimido con los atlas (DDS)
  data/game/01_common_h.sblytbnd   BND4 con los .layout (XML con la posición de cada ícono)

Uso: python tools/extract_icons.py <salida> NOMBRE [NOMBRE...]
  NOMBRE sin extensión, por ejemplo MENU_StatusIcon_20206 o MENU_ItemIcon_19108.
"""
import io
import re
import struct
import sys
from pathlib import Path

from PIL import Image

DATA = Path(__file__).resolve().parent.parent / "data" / "game"


def tpf_textures(tpf: bytes) -> dict[str, bytes]:
    assert tpf[:4] == b"TPF\0"
    (count,) = struct.unpack_from("<i", tpf, 8)
    out = {}
    for i in range(count):
        off, size, _fmt, _typ, _mips, _f1, name_off, _f2 = struct.unpack_from(
            "<2I4BIi", tpf, 0x10 + i * 0x14
        )
        end = name_off
        while tpf[end : end + 2] != b"\0\0":
            end += 2
        out[tpf[name_off:end].decode("utf-16-le")] = tpf[off : off + size]
    return out


def layouts(bnd: bytes) -> dict[str, tuple[str, int, int, int, int]]:
    """nombre del ícono -> (atlas, x, y, ancho, alto)."""
    text = bnd.decode("utf-8", errors="ignore")
    out = {}
    for atlas in re.finditer(r"<TextureAtlas imagePath=\"[^\"]*\\([^\\\"]+)\.tif\"[\s\S]*?</TextureAtlas>", text):
        name = atlas.group(1)
        for sub in re.finditer(r"<SubTexture ([^>]*)/>", atlas.group(0)):
            attrs = dict(re.findall(r'(\w+)="([^"]*)"', sub.group(1)))
            icon = attrs["name"].removesuffix(".png")
            out[icon] = (name, int(attrs["x"]), int(attrs["y"]), int(attrs["width"]), int(attrs["height"]))
    return out


def main() -> None:
    dest = Path(sys.argv[1])
    dest.mkdir(parents=True, exist_ok=True)
    textures = tpf_textures((DATA / "01_common_h.tpf").read_bytes())
    where = layouts((DATA / "01_common_h.sblytbnd").read_bytes())
    atlases: dict[str, Image.Image] = {}
    for name in sys.argv[2:]:
        if name not in where:
            print(f"{name}: no está en los layouts")
            continue
        atlas, x, y, w, h = where[name]
        if atlas not in atlases:
            atlases[atlas] = Image.open(io.BytesIO(textures[atlas])).convert("RGBA")
            print(f"atlas {atlas}: {atlases[atlas].size}")
        atlases[atlas].crop((x, y, x + w, y + h)).save(dest / f"{name}.png")
        print(f"{name}: {atlas} {x},{y} {w}x{h}")


if __name__ == "__main__":
    main()
