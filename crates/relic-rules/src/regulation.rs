//! Lectura de los params directamente del `regulation.bin` del juego, sin CSV exportados.
//!
//! `regulation.bin` = AES-256-CBC (IV en los primeros 16 bytes) → DCX con ZSTD → BND4 con un
//! archivo PARAM por cada tabla. De los PARAM se leen solo los campos que usa el motor, con los
//! offsets de los paramdefs de Nightreign (los de Smithbox, `Assets/PARAM/NR/Defs`).
//!
//! Los nombres de efectos y reliquias no están en los params (están en los textos del juego):
//! acá quedan vacíos.

use std::collections::HashMap;
use std::fmt;

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes256;

use crate::data::{EffectId, EffectInfo, GameData, ItemId, RelicType, TableId};

/// Clave AES de `regulation.bin` de Nightreign (la misma que usan las herramientas de modding).
const REGULATION_KEY: [u8; 32] = [
    0x9a, 0x8e, 0xe9, 0x0c, 0x4c, 0x01, 0xa4, 0x31, 0x68, 0xa1, 0x7d, 0x9d, 0x75, 0xe4, 0xa7, 0xd0,
    0x21, 0x07, 0xeb, 0xcf, 0x43, 0xd5, 0xac, 0xb0, 0x55, 0x4f, 0x94, 0x16, 0x01, 0xb5, 0x79, 0x18,
];

#[derive(Debug)]
pub enum RegulationError {
    Io(std::io::Error),
    Format(String),
}

impl fmt::Display for RegulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegulationError::Io(e) => write!(f, "no pude leer regulation.bin: {e}"),
            RegulationError::Format(e) => {
                write!(f, "regulation.bin con un formato inesperado: {e}")
            }
        }
    }
}

impl std::error::Error for RegulationError {}

fn bad(msg: impl Into<String>) -> RegulationError {
    RegulationError::Format(msg.into())
}

fn u16_le(b: &[u8], o: usize) -> Result<u16, RegulationError> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| bad(format!("lectura fuera de rango en {o:#x}")))
}

fn u32_le(b: &[u8], o: usize) -> Result<u32, RegulationError> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| bad(format!("lectura fuera de rango en {o:#x}")))
}

fn i32_le(b: &[u8], o: usize) -> Result<i32, RegulationError> {
    u32_le(b, o).map(|v| v as i32)
}

fn u64_le(b: &[u8], o: usize) -> Result<u64, RegulationError> {
    b.get(o..o + 8)
        .map(|s| u64::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| bad(format!("lectura fuera de rango en {o:#x}")))
}

fn u32_be(b: &[u8], o: usize) -> Result<u32, RegulationError> {
    b.get(o..o + 4)
        .map(|s| u32::from_be_bytes(s.try_into().unwrap()))
        .ok_or_else(|| bad(format!("lectura fuera de rango en {o:#x}")))
}

/// AES-256-CBC sin padding; el IV son los primeros 16 bytes.
pub fn decrypt(raw: &[u8]) -> Result<Vec<u8>, RegulationError> {
    if raw.len() < 32 || !raw.len().is_multiple_of(16) {
        return Err(bad("el tamaño no es múltiplo de 16"));
    }
    let cipher = Aes256::new(GenericArray::from_slice(&REGULATION_KEY));
    let mut out = Vec::with_capacity(raw.len() - 16);
    for i in (16..raw.len()).step_by(16) {
        let mut block = GenericArray::clone_from_slice(&raw[i..i + 16]);
        cipher.decrypt_block(&mut block);
        let prev = &raw[i - 16..i];
        out.extend(block.iter().zip(prev).map(|(a, b)| a ^ b));
    }
    Ok(out)
}

/// Descomprime un DCX con ZSTD (el formato de Nightreign).
pub fn decompress_dcx(dcx: &[u8]) -> Result<Vec<u8>, RegulationError> {
    if dcx.get(0..4) != Some(b"DCX\0") || dcx.get(0x24..0x28) != Some(b"DCP\0") {
        return Err(bad("no es un DCX"));
    }
    let algo = dcx.get(0x28..0x2C).unwrap_or_default();
    if algo != b"ZSTD" {
        return Err(bad(format!(
            "compresión {} no soportada",
            String::from_utf8_lossy(algo)
        )));
    }
    let size = u32_be(dcx, 0x1C)? as usize;
    let compressed = u32_be(dcx, 0x20)? as usize;
    if dcx.get(0x44..0x48) != Some(b"DCA\0") {
        return Err(bad("falta el bloque DCA"));
    }
    let start = 0x44 + u32_be(dcx, 0x48)? as usize;
    let data = dcx
        .get(start..start + compressed)
        .ok_or_else(|| bad("datos comprimidos truncados"))?;
    let out = zstd::bulk::decompress(data, size).map_err(|e| bad(format!("zstd: {e}")))?;
    if out.len() != size {
        return Err(bad("el tamaño descomprimido no coincide"));
    }
    Ok(out)
}

/// Archivos de un BND4: (nombre, contenido).
pub fn bnd4_files(bnd: &[u8]) -> Result<Vec<(String, &[u8])>, RegulationError> {
    if bnd.get(0..4) != Some(b"BND4") {
        return Err(bad("no es un BND4"));
    }
    if bnd[9] != 0 {
        return Err(bad("BND4 big endian no soportado"));
    }
    let count = i32_le(bnd, 0x0C)? as usize;
    let header_size = u64_le(bnd, 0x20)? as usize;
    let unicode = bnd[0x30] != 0;
    // Diseño de cada cabecera de archivo según su tamaño (el que usa la regulación).
    let (data_off_at, long_offsets, name_at) = match header_size {
        0x24 => (0x18, false, 0x20),
        0x28 => (0x18, true, 0x24),
        other => return Err(bad(format!("cabecera de archivo de {other:#x} bytes"))),
    };
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let h = 0x40 + i * header_size;
        let size = u64_le(bnd, h + 8)? as usize;
        let data_off = if long_offsets {
            u64_le(bnd, h + data_off_at)? as usize
        } else {
            u32_le(bnd, h + data_off_at)? as usize
        };
        let name_off = u32_le(bnd, h + name_at)? as usize;
        let name = read_cstr(bnd, name_off, unicode)?;
        let data = bnd
            .get(data_off..data_off + size)
            .ok_or_else(|| bad(format!("archivo {name} fuera de rango")))?;
        out.push((name, data));
    }
    Ok(out)
}

fn read_cstr(b: &[u8], off: usize, unicode: bool) -> Result<String, RegulationError> {
    if unicode {
        let mut units = Vec::new();
        let mut o = off;
        loop {
            let u = u16_le(b, o)?;
            if u == 0 {
                break;
            }
            units.push(u);
            o += 2;
        }
        Ok(String::from_utf16_lossy(&units))
    } else {
        let end = b[off..]
            .iter()
            .position(|&c| c == 0)
            .ok_or_else(|| bad("texto sin terminar"))?;
        Ok(String::from_utf8_lossy(&b[off..off + end]).into_owned())
    }
}

/// Filas de un archivo PARAM: (ID, datos de la fila). `row_size` sale del paramdef.
pub fn param_rows(param: &[u8], row_size: usize) -> Result<Vec<(i32, &[u8])>, RegulationError> {
    let count = u16_le(param, 0x0A)? as usize;
    let mut rows = Vec::with_capacity(count);
    for i in 0..count {
        let e = 0x40 + i * 0x18;
        let id = i32_le(param, e)?;
        let off = u64_le(param, e + 8)? as usize;
        let data = param
            .get(off..off + row_size)
            .ok_or_else(|| bad(format!("fila {id} fuera de rango")))?;
        rows.push((id, data));
    }
    Ok(rows)
}

// Tamaños de fila y offsets de campos (paramdefs de Nightreign).
const ATTACH_EFFECT_ROW: usize = 0x44;
const ATTACH_EFFECT_TABLE_ROW: usize = 0x0C;
const EQUIP_ANTIQUE_ROW: usize = 0x30;

impl GameData {
    /// Arma los datos del motor desde `regulation.bin` (sin nombres).
    pub fn from_regulation(raw: &[u8]) -> Result<GameData, RegulationError> {
        let bnd = decompress_dcx(&decrypt(raw)?)?;
        let files = bnd4_files(&bnd)?;
        let find = |name: &str| -> Result<&[u8], RegulationError> {
            files
                .iter()
                .find(|(n, _)| n.rsplit(['\\', '/']).next() == Some(name))
                .map(|(_, d)| *d)
                .ok_or_else(|| bad(format!("no está {name}")))
        };
        let mut data = GameData::default();

        for (id, row) in param_rows(find("AttachEffectParam.param")?, ATTACH_EFFECT_ROW)? {
            if id < 0 {
                continue;
            }
            let passive_sp_effects = [0x0C, 0x10, 0x14]
                .into_iter()
                .map(|o| i32_le(row, o))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|&sp| sp > 0)
                .map(|sp| sp as u32)
                .collect();
            data.effects.insert(
                id as EffectId,
                EffectInfo {
                    id: id as EffectId,
                    name: String::new(),
                    compatibility_id: i32_le(row, 0x18)?,
                    sort_priority: i32_le(row, 0x34)?,
                    is_debuff: row[0x21] != 0,
                    passive_sp_effects,
                },
            );
        }

        for (id, row) in param_rows(find("EquipParamAntique.param")?, EQUIP_ANTIQUE_ROW)? {
            if id < 0 {
                continue;
            }
            let t = |o: usize| i32_le(row, o).map(|v| v as TableId);
            data.relic_types.insert(
                id as ItemId,
                RelicType {
                    id: id as ItemId,
                    name: String::new(),
                    is_deep: row[0x07] & 1 != 0,
                    is_salable: row[0x06] != 0,
                    color: row[0x04],
                    slot_tables: [t(0x10)?, t(0x14)?, t(0x18)?],
                    curse_tables: [t(0x20)?, t(0x24)?, t(0x28)?],
                },
            );
        }

        let mut tables: HashMap<TableId, _> = HashMap::new();
        for (id, row) in param_rows(
            find("AttachEffectTableParam.param")?,
            ATTACH_EFFECT_TABLE_ROW,
        )? {
            let effect = i32_le(row, 0x04)?;
            if effect >= 0 {
                tables
                    .entry(id as TableId)
                    .or_insert_with(std::collections::HashSet::new)
                    .insert(effect as EffectId);
            }
        }
        data.tables = tables;
        Ok(data)
    }
}
