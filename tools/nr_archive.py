"""Lee archivos sueltos de los data*.bhd/.bdt de Nightreign (sin desempaquetar todo).

Prueba de concepto antes de pasarlo a Rust. Usa las claves de data/game/nr_bhd_keys.json
(generado con tools/bhd_keys.py).

Uso:
  python tools/nr_archive.py <carpeta Game> get /menu/00_solo_h.tpfbhd salida.bin
  python tools/nr_archive.py <carpeta Game> bnd /menu/00_solo_h.tpfbhd     (lista el BHF4)
"""
import json
import struct
import sys
from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

KEYS = Path(__file__).resolve().parent.parent / "data" / "game" / "nr_bhd_keys.json"


def path_hash(path: str) -> int:
    path = path.lower().replace("\\", "/")
    if not path.startswith("/"):
        path = "/" + path
    h = 0
    for c in path.encode():
        h = (h * 0x85 + c) & 0xFFFFFFFFFFFFFFFF
    return h


def rsa_decrypt(pem: str, data: bytes) -> bytes:
    nums = serialization.load_pem_public_key(pem.encode()).public_numbers()
    size = (nums.n.bit_length() + 7) // 8
    out = bytearray()
    for i in range(0, len(data), size):
        m = pow(int.from_bytes(data[i : i + size], "big"), nums.e, nums.n)
        out += m.to_bytes(size - 1, "big")
    return bytes(out)


class Archive:
    def __init__(self, game: Path, name: str, pem: str):
        self.bdt = game / f"{name}.bdt"
        bhd = rsa_decrypt(pem, (game / f"{name}.bhd").read_bytes())
        assert bhd[:4] == b"BHD5", bhd[:16]
        _endian, _unk, _size, bucket_count, buckets_off = struct.unpack_from("<5i", bhd, 4)
        self.files = {}
        for b in range(bucket_count):
            count, off = struct.unpack_from("<2i", bhd, buckets_off + b * 8)
            for f in range(count):
                h, padded, unpadded, offset, _salt, aes = struct.unpack_from(
                    "<Q2i3q", bhd, off + f * 40
                )
                ranges = []
                key = None
                if aes:
                    key = bhd[aes : aes + 16]
                    (n,) = struct.unpack_from("<i", bhd, aes + 16)
                    for r in range(n):
                        ranges.append(struct.unpack_from("<2q", bhd, aes + 20 + r * 16))
                self.files[h] = (padded, unpadded, offset, key, ranges)

    def read(self, path: str) -> bytes | None:
        entry = self.files.get(path_hash(path))
        if entry is None:
            return None
        padded, unpadded, offset, key, ranges = entry
        with open(self.bdt, "rb") as f:
            f.seek(offset)
            data = bytearray(f.read(padded))
        if key:
            dec = Cipher(algorithms.AES(key), modes.ECB()).decryptor()
            for start, end in ranges:
                if start == -1 or end <= start:
                    continue
                data[start:end] = dec.update(bytes(data[start:end]))
        return bytes(data[:unpadded] if unpadded else data)


def open_all(game: Path) -> list[Archive]:
    keys = json.loads(KEYS.read_text())
    return [Archive(game, name, pem) for name, pem in sorted(keys.items())]


def read(archives: list[Archive], path: str) -> bytes:
    for a in archives:
        data = a.read(path)
        if data is not None:
            return data
    raise FileNotFoundError(path)


def dcx_decompress(game: Path, data: bytes) -> bytes:
    """DCX con KRAK (Oodle): usa la oo2core_9_win64.dll que trae el juego."""
    import ctypes

    assert data[:4] == b"DCX\0", data[:4]
    assert data[0x28:0x2C] == b"KRAK", data[0x28:0x2C]
    (size,) = struct.unpack_from(">i", data, 0x1C)
    (packed,) = struct.unpack_from(">i", data, 0x20)
    src = data[0x4C : 0x4C + packed]
    oodle = ctypes.WinDLL(str(game / "oo2core_9_win64.dll"))
    fn = oodle.OodleLZ_Decompress
    fn.restype = ctypes.c_int64
    fn.argtypes = [ctypes.c_char_p, ctypes.c_int64, ctypes.c_char_p, ctypes.c_int64] + [
        ctypes.c_int
    ] * 3 + [ctypes.c_void_p] * 6 + [ctypes.c_int]
    out = ctypes.create_string_buffer(size)
    # fuzzSafe, checkCRC, verbosity, decBufBase, decBufSize, callback, userdata,
    # decoderMemory, decoderMemorySize, threadPhase
    n = fn(src, len(src), out, size, 1, 0, 0, None, None, None, None, None, None, 3)
    assert n == size, (n, size)
    return out.raw


def bhf4_entries(bhd: bytes) -> list[tuple[str, int, int]]:
    """(nombre, tamaño, offset en el .bdt) de un BHF4 (encabezado de un binder partido)."""
    assert bhd[:4] == b"BHF4", bhd[:4]
    (count,) = struct.unpack_from("<i", bhd, 0x0C)
    (header_size,) = struct.unpack_from("<q", bhd, 0x20)
    unicode = bhd[0x30]
    # Entradas de 0x24 bytes (formato 0x74): flags, -1, tamaño, tamaño sin comprimir,
    # offset (i32), id, offset del nombre. Visto en 00_solo_h.tpfbhd.
    assert header_size == 0x24, header_size
    entries = []
    pos = 0x40
    for _ in range(count):
        e = bhd[pos : pos + header_size]
        flags = e[0]
        size = struct.unpack_from("<q", e, 8)[0]
        offset, _id, name_off = struct.unpack_from("<3i", e, 0x18)
        end = name_off
        if unicode:
            while bhd[end : end + 2] != b"\0\0":
                end += 2
            name = bhd[name_off:end].decode("utf-16-le")
        else:
            end = bhd.index(b"\0", name_off)
            name = bhd[name_off:end].decode("shift-jis")
        entries.append((name, size, offset, flags))
        pos += header_size
    return entries


def main() -> None:
    game, cmd, path = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
    archives = open_all(game)
    data = read(archives, path)
    if cmd == "get":
        if data[:4] == b"DCX\0":
            data = dcx_decompress(game, data)
        Path(sys.argv[4]).write_bytes(data)
        print(f"{len(data)} bytes, empieza con {data[:8]!r}")
    elif cmd == "bnd":
        for e in bhf4_entries(data):
            print(*e)


if __name__ == "__main__":
    main()
