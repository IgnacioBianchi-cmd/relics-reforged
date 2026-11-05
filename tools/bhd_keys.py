"""Busca, entre las claves RSA públicas que trae Andre.Formats.dll (Smithbox), las que
descifran los .bhd de Nightreign (el encabezado descifrado empieza con "BHD5").

Uso: python -I tools/bhd_keys.py <Andre.Formats.dll> <carpeta Game> [salida.json]
"""
import json
import re
import sys
from pathlib import Path

from cryptography.hazmat.primitives import serialization


def keys_from_dll(path: Path) -> list[str]:
    raw = path.read_bytes()
    pat = r"-----BEGIN RSA PUBLIC KEY-----[\s\S]{100,800}?-----END RSA PUBLIC KEY-----"
    found = []
    # Las cadenas del heap #US de .NET pueden caer en offset par o impar.
    for start in (0, 1):
        text = raw[start:].decode("utf-16-le", errors="ignore")
        found += re.findall(pat, text)
    return list(dict.fromkeys(found))


def rsa_block(key, block: bytes) -> bytes:
    nums = key.public_numbers()
    size = (nums.n.bit_length() + 7) // 8
    m = pow(int.from_bytes(block[:size], "big"), nums.e, nums.n)
    if m.bit_length() > (size - 1) * 8:
        return b""
    return m.to_bytes(size - 1, "big")


def main() -> None:
    dll, game = Path(sys.argv[1]), Path(sys.argv[2])
    pems = keys_from_dll(dll)
    print(f"{len(pems)} claves en la DLL")
    found = {}
    for bhd in sorted(game.glob("*.bhd")):
        first = bhd.read_bytes()[:1024]
        for i, pem in enumerate(pems):
            key = serialization.load_pem_public_key(pem.encode())
            out = rsa_block(key, first)
            if out[:4] == b"BHD5":
                print(f"{bhd.name}: clave {i}")
                found[bhd.stem] = pem
                break
        else:
            print(f"{bhd.name}: ninguna")
    if len(sys.argv) > 3:
        Path(sys.argv[3]).write_text(json.dumps(found, indent=1))


if __name__ == "__main__":
    main()
