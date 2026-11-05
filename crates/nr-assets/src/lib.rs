//! Lectura de archivos del juego empaquetados en `data*.bhd/.bdt`, para sacar los íconos del
//! menú del juego instalado del jugador (el mod no reparte imágenes del juego).
//!
//! Cadena (verificada con `tools/nr_archive.py`):
//! - `dataN.bhd`: RSA (clave pública, bloques de 256 → 255 bytes) → `BHD5` con el índice:
//!   hash de la ruta (64 bits) → tamaño, offset en `dataN.bdt` y rangos cifrados con AES-128-ECB.
//! - Los archivos `.dcx` vienen comprimidos con Oodle Kraken: se usa la `oo2core_9_win64.dll`
//!   que trae el juego.
//! - `menu/01_common_h.tpf.dcx`: TPF con los atlas (DDS en BC7). `menu/01_common_h.sblytbnd.dcx`:
//!   BND4 con XML que dicen dónde está cada ícono dentro de cada atlas.

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes128;
use num_bigint::BigUint;

#[derive(Debug)]
pub struct AssetError(pub String);

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AssetError {}

type Result<T> = std::result::Result<T, AssetError>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(AssetError(msg.into()))
}

fn rd<const N: usize>(b: &[u8], o: usize) -> Result<[u8; N]> {
    b.get(o..o + N)
        .map(|s| s.try_into().unwrap())
        .ok_or_else(|| AssetError(format!("lectura fuera de rango en {o:#x}")))
}
fn i32_le(b: &[u8], o: usize) -> Result<i32> {
    rd::<4>(b, o).map(i32::from_le_bytes)
}
fn u32_le(b: &[u8], o: usize) -> Result<u32> {
    rd::<4>(b, o).map(u32::from_le_bytes)
}
fn i64_le(b: &[u8], o: usize) -> Result<i64> {
    rd::<8>(b, o).map(i64::from_le_bytes)
}
fn u64_le(b: &[u8], o: usize) -> Result<u64> {
    rd::<8>(b, o).map(u64::from_le_bytes)
}
fn u32_be(b: &[u8], o: usize) -> Result<u32> {
    rd::<4>(b, o).map(u32::from_be_bytes)
}

/// Claves RSA públicas de los `.bhd` de Nightreign (las mismas que usan las herramientas de
/// modding; están en Andre.Formats de Smithbox). Se ubicaron con `tools/bhd_keys.py`.
const KEYS: [(&str, &str); 5] = [
    ("data0", "MIIBDAKCAQEAz8F9U1V9hgKs40gdzl1ZOf3IBirf6xUEzXtDd6oSEBE6XiYocvABykiK+WMdAaJL7HJ58Gt2xSRxA3t9toCGKMI/3gNAfcR0BV83gsQo0O0dVP0fqyxXlA2pGN5B4IE8aLWPX2cNNFSFKAdjYnzsYSevzef/pgnpV1ZgPf2j2SQwNGSufYeN3Owji8l0K2C0fKIx6gSO0cK9kvTIm8AdpvzZbBkTylT1jF3m8DsSA1OFzFJTdFyZbTRi85M6bmv6rHtvZc5OW21dye7Q6fmLlxOyMetLTu4dpOXjHAAf/LFTbfQpXFr9aXO4O6I7nWDJn7FRzNlLkb8RwSyZ1/KWyQIFALEDsAc="),
    ("data1", "MIIBDAKCAQEA0E6dtnDmT6d2+VaNkPzomUNv+T6896H//RAaTR2guPACMDNZpAsFvV3MfNcR2BS6Cbxl55MmMWsmsZs1s293MuOdS+c99vmZbNYcXWjx0uJGO+VrRXe43TRzmQFh1uD+Xcq6+wYfTrGyLOdAtmwdDXNvW8jYoFDM7nsuoPKOXKtKd0uz7/MKZYLk1J7pAoBQqw9VD5qi2Ih86zn0VWm5lLMTI0qnutOzpZVDvZWBg/jr4Nbnr/OxPLeJO1tFuRuHUPuBAWtYM/J23MPqqKkQrG5z2r7PexUI744UPdmo3Sn+MqynuxxvV9SEhska6pStzn8R9i94wOKPTQ32HEFuUQIFAP////8="),
    ("data2", "MIIBDAKCAQEAqpkf9yHnx8k84+WXITLFUW/STypXjZMPuw842pzNHa5L7v9gU4M5hBHwTQs0YIcfnf+mbjqoJYnmYPBblxLjFXgwT4ICJdpnPMY75BwD0Nv28/CvvIsA0QQWOhUeOXnm5BT26dGYi3CHHPvD14F76tJt3TO/CC3fyhdxne9Cra5G87aGTJGv0ImsU0KPCizYX/RHQ2jdJdlB5BHzkMgLhIaEdhC3nhIqMJDNQNGKMo7rRV1tAEGf0zIZ23PGEsPsbVg31nnnRoq338WfD9ArZZG6bM11vlfVcYmrJs7v4vBjKXnYVwVX0rQGIfSNDnaZcEj4tsl04AqnupTdvSrHXwIFANOg6RU="),
    ("data3", "MIIBCwKCAQEAwm2Rcw4eoP8FgWijxw1X8b9rEVFsVqy7rXWcH2yVm61yYBlzPlTqKqnc2VeqZSh/TLXeFY3+Om2X78RQxZNS3L3OokvD7l/0wqPIpXSSumeeL8UAZm5k7nFA2m2HJfc+F07kNwwCEqhmFs5YQIMnWyIrqnEax/qSncFErLjIYMBMArVnVLE8WqgsD7N8lW937dlUcT2TaPh1HfjavKOSUy/OHM9zaneyDL4NRmDdU8GmNXTSm5kPYoSRCDIvFVj0g5iaXr60eRh0d+40TctoBUdtaoJCPOyRlmkE7qU6Q9FyyvMNbhtfD95d+6IJejNd7kvyV/ISlB37kb2Uh9TavwIEOqKLtw=="),
    ("dlc01", "MIIBDAKCAQEA1q4MOehlD++h5Ietq9Jk97eGOJL2zDpDcu9Wk6RXK1+R3LycMBQlL/hnPg/qqvcoViA7wLX5GOFr5lo6dtKaQqlBkBqgYHGIdBvioBPZ8BuXAjYr3sm8N0SYC2TNHXmfw6yFC+ePsrl+gNldrO//XXY27hsGgcegfWr6JuQaJti/BOKlGb8ARbKwyIqGc5WiWj/v0tGE1cdPi0fLQRbTrLFaQtx1roQVqsQuJ5zRGTpnj/mhaJtqJ7V0s5gLG5CCevx71lN8m7oyWk2JemzSLvllwv4tjtzrw3jNQtiYb8nzy2SpjibsvX1iRCg5btMSiNPcSeIJ5jX+FUW9LSnrkwIFAKhopbM="),
];

fn base64(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b' ' | b'\n' | b'\r' => continue,
            _ => return err("base64 inválido"),
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// `RSAPublicKey ::= SEQUENCE { modulus INTEGER, publicExponent INTEGER }` (PKCS#1, DER).
fn parse_key(der: &[u8]) -> Result<(BigUint, BigUint)> {
    fn element(b: &[u8], tag: u8) -> Result<(&[u8], &[u8])> {
        if b.first() != Some(&tag) {
            return err("clave RSA: etiqueta DER inesperada");
        }
        let (len, head) = match *b.get(1).ok_or(AssetError("clave RSA corta".into()))? {
            n if n < 0x80 => (n as usize, 2),
            n => {
                let k = (n & 0x7F) as usize;
                let len = b
                    .get(2..2 + k)
                    .ok_or(AssetError("clave RSA corta".into()))?
                    .iter()
                    .fold(0usize, |a, &x| (a << 8) | x as usize);
                (len, 2 + k)
            }
        };
        let body = b
            .get(head..head + len)
            .ok_or(AssetError("clave RSA corta".into()))?;
        Ok((body, &b[head + len..]))
    }
    let (seq, _) = element(der, 0x30)?;
    let (n, rest) = element(seq, 0x02)?;
    let (e, _) = element(rest, 0x02)?;
    Ok((BigUint::from_bytes_be(n), BigUint::from_bytes_be(e)))
}

fn rsa_decrypt(key_b64: &str, data: &[u8]) -> Result<Vec<u8>> {
    let (n, e) = parse_key(&base64(key_b64)?)?;
    let size = n.bits().div_ceil(8) as usize;
    let mut out = Vec::with_capacity(data.len());
    for block in data.chunks(size) {
        let m = BigUint::from_bytes_be(block).modpow(&e, &n).to_bytes_be();
        if m.len() > size - 1 {
            return err("bloque RSA inválido");
        }
        out.resize(out.len() + size - 1 - m.len(), 0);
        out.extend_from_slice(&m);
    }
    Ok(out)
}

/// Hash de ruta de Elden Ring / Nightreign (64 bits).
pub fn path_hash(path: &str) -> u64 {
    let mut path = path.to_ascii_lowercase().replace('\\', "/");
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    path.bytes()
        .fold(0u64, |h, c| h.wrapping_mul(0x85).wrapping_add(c as u64))
}

/// Clave AES-128 de un archivo y los rangos (inicio, fin) cifrados con ella.
type AesRanges = ([u8; 16], Vec<(i64, i64)>);

struct Entry {
    padded: usize,
    unpadded: usize,
    offset: u64,
    aes: Option<AesRanges>,
}

struct Archive {
    bdt: PathBuf,
    files: HashMap<u64, Entry>,
}

impl Archive {
    fn open(game: &Path, name: &str, key: &str) -> Result<Archive> {
        let raw = std::fs::read(game.join(format!("{name}.bhd")))
            .map_err(|e| AssetError(format!("{name}.bhd: {e}")))?;
        let bhd = rsa_decrypt(key, &raw)?;
        if bhd.get(0..4) != Some(b"BHD5") {
            return err(format!("{name}.bhd no descifró a BHD5"));
        }
        let buckets = i32_le(&bhd, 0x10)? as usize;
        let buckets_off = i32_le(&bhd, 0x14)? as usize;
        let mut files = HashMap::new();
        for b in 0..buckets {
            let count = i32_le(&bhd, buckets_off + b * 8)? as usize;
            let off = i32_le(&bhd, buckets_off + b * 8 + 4)? as usize;
            for f in 0..count {
                let h = off + f * 40;
                let aes_off = i64_le(&bhd, h + 32)? as usize;
                let aes = if aes_off != 0 {
                    let key = rd::<16>(&bhd, aes_off)?;
                    let n = i32_le(&bhd, aes_off + 16)? as usize;
                    let ranges = (0..n)
                        .map(|r| {
                            let o = aes_off + 20 + r * 16;
                            Ok((i64_le(&bhd, o)?, i64_le(&bhd, o + 8)?))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Some((key, ranges))
                } else {
                    None
                };
                files.insert(
                    u64_le(&bhd, h)?,
                    Entry {
                        padded: i32_le(&bhd, h + 8)? as usize,
                        unpadded: i32_le(&bhd, h + 12)? as usize,
                        offset: i64_le(&bhd, h + 16)? as u64,
                        aes,
                    },
                );
            }
        }
        Ok(Archive {
            bdt: game.join(format!("{name}.bdt")),
            files,
        })
    }

    fn read(&self, hash: u64) -> Option<Result<Vec<u8>>> {
        let e = self.files.get(&hash)?;
        Some((|| {
            let mut f = File::open(&self.bdt).map_err(|x| AssetError(x.to_string()))?;
            f.seek(SeekFrom::Start(e.offset))
                .map_err(|x| AssetError(x.to_string()))?;
            let mut data = vec![0; e.padded];
            f.read_exact(&mut data)
                .map_err(|x| AssetError(x.to_string()))?;
            if let Some((key, ranges)) = &e.aes {
                let cipher = Aes128::new(GenericArray::from_slice(key));
                for &(start, end) in ranges {
                    if start < 0 || end <= start || end as usize > data.len() {
                        continue;
                    }
                    let (blocks, _) = data[start as usize..end as usize].as_chunks_mut::<16>();
                    for block in blocks {
                        cipher.decrypt_block(GenericArray::from_mut_slice(block));
                    }
                }
            }
            if e.unpadded > 0 && e.unpadded <= data.len() {
                data.truncate(e.unpadded);
            }
            Ok(data)
        })())
    }
}

/// Los archivos empaquetados del juego.
pub struct GameFiles {
    game: PathBuf,
    archives: Vec<Archive>,
}

impl GameFiles {
    /// `game` = carpeta de `nightreign.exe`. Abre solo los índices (no lee los `.bdt`).
    pub fn open(game: &Path) -> Result<GameFiles> {
        let archives = KEYS
            .iter()
            .filter(|(name, _)| game.join(format!("{name}.bhd")).exists())
            .map(|(name, key)| Archive::open(game, name, key))
            .collect::<Result<Vec<_>>>()?;
        if archives.is_empty() {
            return err("no encontré data0.bhd");
        }
        Ok(GameFiles {
            game: game.to_path_buf(),
            archives,
        })
    }

    /// Un archivo por su ruta (por ejemplo `/menu/01_common_h.tpf.dcx`), sin descomprimir.
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let hash = path_hash(path);
        self.archives
            .iter()
            .find_map(|a| a.read(hash))
            .unwrap_or_else(|| err(format!("no está {path}")))
    }

    /// Un archivo `.dcx`, descomprimido.
    pub fn read_dcx(&self, path: &str) -> Result<Vec<u8>> {
        let dcx = self.read(path)?;
        decompress_dcx(&self.game, &dcx)
    }
}

type OodleDecompress = unsafe extern "system" fn(
    *const u8,
    i64,
    *mut u8,
    i64,
    i32,
    i32,
    i32,
    *mut u8,
    i64,
    *const u8,
    *const u8,
    *mut u8,
    i64,
    i32,
) -> i64;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut u8;
    fn GetProcAddress(module: *mut u8, name: *const u8) -> *mut u8;
}

/// `OodleLZ_Decompress` de la `oo2core_9_win64.dll` del juego (si el juego ya la cargó,
/// `LoadLibraryW` devuelve el mismo módulo).
fn oodle(game: &Path) -> Result<OodleDecompress> {
    let dll: Vec<u16> = game
        .join("oo2core_9_win64.dll")
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain([0])
        .collect();
    unsafe {
        let module = LoadLibraryW(dll.as_ptr());
        if module.is_null() {
            return err("no pude cargar oo2core_9_win64.dll");
        }
        let f = GetProcAddress(module, c"OodleLZ_Decompress".as_ptr().cast());
        if f.is_null() {
            return err("oo2core_9_win64.dll sin OodleLZ_Decompress");
        }
        Ok(std::mem::transmute::<*mut u8, OodleDecompress>(f))
    }
}

/// DCX con Kraken (Oodle).
pub fn decompress_dcx(game: &Path, dcx: &[u8]) -> Result<Vec<u8>> {
    if dcx.get(0..4) != Some(b"DCX\0") || dcx.get(0x24..0x28) != Some(b"DCP\0") {
        return err("no es un DCX");
    }
    if dcx.get(0x28..0x2C) != Some(b"KRAK") {
        return err("DCX con una compresión distinta de KRAK");
    }
    let size = u32_be(dcx, 0x1C)? as usize;
    let packed = u32_be(dcx, 0x20)? as usize;
    if dcx.get(0x44..0x48) != Some(b"DCA\0") {
        return err("DCX sin bloque DCA");
    }
    let start = 0x44 + u32_be(dcx, 0x48)? as usize;
    let src = dcx
        .get(start..start + packed)
        .ok_or(AssetError("DCX truncado".into()))?;
    let decompress = oodle(game)?;
    let mut out = vec![0u8; size];
    let n = unsafe {
        decompress(
            src.as_ptr(),
            src.len() as i64,
            out.as_mut_ptr(),
            size as i64,
            1,
            0,
            0,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
            3,
        )
    };
    if n != size as i64 {
        return err(format!("Oodle devolvió {n} de {size} bytes"));
    }
    Ok(out)
}

fn utf16_cstr(b: &[u8], mut o: usize) -> Result<String> {
    let mut units = Vec::new();
    loop {
        let u = u16::from_le_bytes(rd::<2>(b, o)?);
        if u == 0 {
            return Ok(String::from_utf16_lossy(&units));
        }
        units.push(u);
        o += 2;
    }
}

/// Texturas (DDS completos) de un TPF de PC, por nombre.
pub fn tpf_textures(tpf: &[u8]) -> Result<HashMap<String, &[u8]>> {
    if tpf.get(0..4) != Some(b"TPF\0") {
        return err("no es un TPF");
    }
    let count = i32_le(tpf, 8)? as usize;
    let mut out = HashMap::new();
    for i in 0..count {
        let e = 0x10 + i * 0x14;
        let off = u32_le(tpf, e)? as usize;
        let size = u32_le(tpf, e + 4)? as usize;
        let name = utf16_cstr(tpf, u32_le(tpf, e + 12)? as usize)?;
        let data = tpf
            .get(off..off + size)
            .ok_or_else(|| AssetError(format!("textura {name} fuera de rango")))?;
        out.insert(name, data);
    }
    Ok(out)
}

/// Dónde está un ícono: atlas (nombre de la textura en el TPF) y rectángulo en píxeles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite {
    pub atlas: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// Íconos de los `.layout` (XML) de un `.sblytbnd`: nombre sin `.png` → [`Sprite`].
pub fn layout_sprites(bnd: &[u8]) -> HashMap<String, Sprite> {
    let text = String::from_utf8_lossy(bnd);
    let mut out = HashMap::new();
    for atlas in text.split("<TextureAtlas ").skip(1) {
        let Some(path) = attr(&format!(" {atlas}"), "imagePath").map(str::to_string) else {
            continue;
        };
        let name = path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&path)
            .trim_end_matches(".tif")
            .to_string();
        let body = atlas.split("</TextureAtlas>").next().unwrap_or("");
        for sub in body.split("<SubTexture").skip(1) {
            let tag = sub.split("/>").next().unwrap_or("");
            let num = |k: &str| attr(tag, k).and_then(|v| v.parse::<u32>().ok());
            if let (Some(icon), Some(x), Some(y), Some(w), Some(h)) = (
                attr(tag, "name"),
                num("x"),
                num("y"),
                num("width"),
                num("height"),
            ) {
                out.insert(
                    icon.trim_end_matches(".png").to_string(),
                    Sprite {
                        atlas: name.clone(),
                        x,
                        y,
                        w,
                        h,
                    },
                );
            }
        }
    }
    out
}

/// Recorta y decodifica un rectángulo de un DDS en BC7 a RGBA (8 bits por canal).
pub fn decode_bc7_crop(dds: &[u8], x: u32, y: u32, w: u32, h: u32) -> Result<Vec<u8>> {
    if dds.get(0..4) != Some(b"DDS ") || dds.get(84..88) != Some(b"DX10") {
        return err("DDS sin encabezado DX10");
    }
    let format = u32_le(dds, 128)?;
    if format != 98 && format != 99 {
        return err(format!("DDS en formato DXGI {format}, se esperaba BC7"));
    }
    let height = u32_le(dds, 12)?;
    let width = u32_le(dds, 16)?;
    if x + w > width || y + h > height {
        return err("recorte fuera de la textura");
    }
    // Se decodifican los bloques de 4x4 que cubren el rectángulo y después se recorta.
    let (bx0, by0) = (x / 4, y / 4);
    let (bx1, by1) = ((x + w).div_ceil(4), (y + h).div_ceil(4));
    let (aw, ah) = ((bx1 - bx0) * 4, (by1 - by0) * 4);
    let data = &dds[148..];
    let row_blocks = width.div_ceil(4) as usize;
    let mut blocks = Vec::with_capacity((aw * ah) as usize);
    for by in by0..by1 {
        let start = (by as usize * row_blocks + bx0 as usize) * 16;
        let end = start + (bx1 - bx0) as usize * 16;
        blocks.extend_from_slice(
            data.get(start..end)
                .ok_or(AssetError("DDS truncado".into()))?,
        );
    }
    let mut pixels = vec![0u32; (aw * ah) as usize];
    texture2ddecoder::decode_bc7(&blocks, aw as usize, ah as usize, &mut pixels)
        .map_err(|e| AssetError(format!("BC7: {e}")))?;
    let (ox, oy) = (x - bx0 * 4, y - by0 * 4);
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = ((oy + row) * aw + ox) as usize;
        // texture2ddecoder entrega 0xAARRGGBB.
        out.extend(pixels[start..start + w as usize].iter().flat_map(|p| {
            let [b, g, r, a] = p.to_le_bytes();
            [r, g, b, a]
        }));
    }
    Ok(out)
}

/// Un atlas propio con los íconos pedidos, listo para subir como una sola textura.
pub struct IconAtlas {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Nombre del ícono → rectángulo en píxeles dentro de `rgba` (x, y, ancho, alto).
    pub rects: HashMap<String, [u32; 4]>,
}

/// Lee los íconos del menú del juego (`MENU_StatusIcon_…`, `MENU_ItemIcon_…`, `KG_Key_…`) y los
/// junta en un atlas. Los nombres que no existen se ignoran (quedan fuera de `rects`).
pub fn load_menu_icons(game: &Path, wanted: &[String]) -> Result<IconAtlas> {
    let files = GameFiles::open(game)?;
    let sprites = layout_sprites(&files.read_dcx("/menu/01_common_h.sblytbnd.dcx")?);
    let tpf = files.read_dcx("/menu/01_common_h.tpf.dcx")?;
    let textures = tpf_textures(&tpf)?;

    // Acomoda los íconos en filas de un atlas de 2048 de ancho.
    const WIDTH: u32 = 2048;
    let mut found: Vec<(&String, &Sprite)> = wanted
        .iter()
        .filter_map(|n| sprites.get(n).map(|s| (n, s)))
        .collect();
    found.sort_by(|a, b| b.1.h.cmp(&a.1.h).then(a.0.cmp(b.0)));
    found.dedup_by(|a, b| a.0 == b.0);
    let (mut cx, mut cy, mut row_h) = (0u32, 0u32, 0u32);
    let mut placed = Vec::with_capacity(found.len());
    for (name, s) in found {
        if cx + s.w > WIDTH {
            cx = 0;
            cy += row_h;
            row_h = 0;
        }
        placed.push((name.clone(), s, cx, cy));
        cx += s.w;
        row_h = row_h.max(s.h);
    }
    let height = (cy + row_h).max(4);
    let mut rgba = vec![0u8; (WIDTH * height * 4) as usize];
    let mut rects = HashMap::new();
    for (name, s, px, py) in placed {
        let Some(dds) = textures.get(&s.atlas) else {
            continue;
        };
        let pixels = decode_bc7_crop(dds, s.x, s.y, s.w, s.h)?;
        for row in 0..s.h {
            let src = (row * s.w * 4) as usize;
            let dst = (((py + row) * WIDTH + px) * 4) as usize;
            rgba[dst..dst + (s.w * 4) as usize]
                .copy_from_slice(&pixels[src..src + (s.w * 4) as usize]);
        }
        rects.insert(name, [px, py, s.w, s.h]);
    }
    Ok(IconAtlas {
        width: WIDTH,
        height,
        rgba,
        rects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_de_ruta() {
        // Mismo resultado que tools/nr_archive.py (sin distinguir mayúsculas ni barras).
        assert_eq!(path_hash("/menu/a"), path_hash("MENU\\A"));
        assert_eq!(path_hash("a"), (b'/' as u64) * 0x85 + b'a' as u64);
    }

    #[test]
    fn base64_basico() {
        assert_eq!(base64("TWFu").unwrap(), b"Man");
        assert_eq!(base64("TWE=").unwrap(), b"Ma");
    }

    #[test]
    fn sprites_de_un_layout() {
        let xml = br#"<TextureAtlas imagePath="W:\x\SB_Icon_0.tif" width="64" height="64">
<SubTexture name="MENU_StatusIcon_1.png" x="4" width="8" y="12" height="16" half="0"/>
</TextureAtlas>"#;
        let s = layout_sprites(xml);
        assert_eq!(
            s["MENU_StatusIcon_1"],
            Sprite {
                atlas: "SB_Icon_0".into(),
                x: 4,
                y: 12,
                w: 8,
                h: 16
            }
        );
    }
}
