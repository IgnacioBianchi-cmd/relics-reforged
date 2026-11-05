//! Datos del juego que necesita el motor: efectos, tipos de reliquia y tablas de roll.
//!
//! Se pueden cargar desde los CSV exportados con Smithbox ([`GameData::from_csv_dir`]) o
//! construir a mano (tests, o la DLL leyendo los params desde la memoria del juego).

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

/// ID de fila de `AttachEffectParam` (efecto o maldición).
pub type EffectId = u32;
/// ID de fila de `EquipParamAntique` (tipo de reliquia). Es el `item_id` del save.
pub type ItemId = u32;
/// ID de tabla de `AttachEffectTableParam`. `-1` = slot sin tabla.
pub type TableId = i32;

/// Valor que usa el juego para "sin efecto" en el save y en memoria.
pub const EMPTY: u32 = 0xFFFF_FFFF;

/// Una fila de `AttachEffectParam` reducida a lo que usan las reglas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectInfo {
    pub id: EffectId,
    pub name: String,
    /// Dos efectos con el mismo valor (distinto de -1) no pueden ir en la misma reliquia.
    pub compatibility_id: i32,
    /// Campo `overrideEffectId`: el juego ordena los efectos por (este valor, ID).
    pub sort_priority: i32,
    pub is_debuff: bool,
    /// SpEffects que aplica el efecto (`passiveSpEffectId_1..3`, sin los vacíos).
    pub passive_sp_effects: Vec<u32>,
    /// ID del nombre en los textos del juego (`attachTextId`).
    pub text_id: i32,
    /// Ícono del efecto en los menús (`statusIconId`): `MENU_StatusIcon_<id>`. -1 = sin ícono.
    pub icon_id: i32,
}

/// Una fila de `EquipParamAntique` reducida a lo que usan las reglas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelicType {
    pub id: ItemId,
    pub name: String,
    pub is_deep: bool,
    pub is_salable: bool,
    pub color: u8,
    /// Tabla de efectos de cada slot (`attachEffectTableId_1..3`).
    pub slot_tables: [TableId; 3],
    /// Tabla de maldición de cada slot (`attachEffectTableId_curse1..3`).
    pub curse_tables: [TableId; 3],
    /// Ícono de la reliquia (`iconId`): `MENU_ItemIcon_<id>`.
    pub icon_id: i32,
}

/// Todos los datos del juego que necesita el motor.
#[derive(Debug, Default, Clone)]
pub struct GameData {
    pub effects: HashMap<EffectId, EffectInfo>,
    pub relic_types: HashMap<ItemId, RelicType>,
    /// Efectos presentes en cada tabla (con cualquier peso: si el juego lo generó alguna vez, es legal).
    pub tables: HashMap<TableId, HashSet<EffectId>>,
}

impl GameData {
    /// ¿El efecto `effect` figura en la tabla `table`?
    pub fn table_contains(&self, table: TableId, effect: EffectId) -> bool {
        table >= 0 && self.tables.get(&table).is_some_and(|s| s.contains(&effect))
    }

    /// Cantidad de efectos distintos en una tabla (0 si no existe).
    pub fn table_len(&self, table: TableId) -> usize {
        if table < 0 {
            return 0;
        }
        self.tables.get(&table).map_or(0, |s| s.len())
    }

    /// Clave con la que el juego ordena los efectos dentro de una reliquia.
    /// Los efectos desconocidos van al final.
    pub fn sort_key(&self, effect: EffectId) -> (i32, EffectId) {
        match self.effects.get(&effect) {
            Some(info) => (info.sort_priority, effect),
            None => (i32::MAX, effect),
        }
    }

    /// Carga `AttachEffectParam.csv`, `EquipParamAntique.csv` y `AttachEffectTableParam.csv`
    /// desde una carpeta (formato de exportación de Smithbox: todas las filas, todos los campos).
    pub fn from_csv_dir(dir: impl AsRef<Path>) -> Result<Self, DataError> {
        let dir = dir.as_ref();
        let mut data = GameData::default();

        for_each_row(&dir.join("AttachEffectParam.csv"), |row| {
            let id = row.int("ID")?;
            if id >= 0 {
                let mut passive_sp_effects = Vec::new();
                for col in [
                    "passiveSpEffectId_1",
                    "passiveSpEffectId_2",
                    "passiveSpEffectId_3",
                ] {
                    let sp = row.int(col)?;
                    if sp > 0 {
                        passive_sp_effects.push(sp as u32);
                    }
                }
                data.effects.insert(
                    id as EffectId,
                    EffectInfo {
                        id: id as EffectId,
                        name: row.text("Name").to_string(),
                        compatibility_id: row.int("compatibilityId")? as i32,
                        sort_priority: row.int("overrideEffectId")? as i32,
                        is_debuff: row.int("isDebuff")? != 0,
                        passive_sp_effects,
                        text_id: row.int("attachTextId")? as i32,
                        icon_id: row.int("statusIconId")? as i32,
                    },
                );
            }
            Ok(())
        })?;

        for_each_row(&dir.join("EquipParamAntique.csv"), |row| {
            let id = row.int("ID")?;
            if id >= 0 {
                let t = |name: &str| row.int(name).map(|v| v as TableId);
                data.relic_types.insert(
                    id as ItemId,
                    RelicType {
                        id: id as ItemId,
                        name: row.text("Name").to_string(),
                        is_deep: row.int("isDeepRelic")? != 0,
                        is_salable: row.int("isSalable")? != 0,
                        color: row.int("relicColor")? as u8,
                        slot_tables: [
                            t("attachEffectTableId_1")?,
                            t("attachEffectTableId_2")?,
                            t("attachEffectTableId_3")?,
                        ],
                        curse_tables: [
                            t("attachEffectTableId_curse1")?,
                            t("attachEffectTableId_curse2")?,
                            t("attachEffectTableId_curse3")?,
                        ],
                        icon_id: row.int("iconId")? as i32,
                    },
                );
            }
            Ok(())
        })?;

        for_each_row(&dir.join("AttachEffectTableParam.csv"), |row| {
            let table = row.int("ID")? as TableId;
            let effect = row.int("attachEffectId")?;
            if effect >= 0 {
                // las entradas con -1 son el relleno "vacío" de la tabla 0
                data.tables
                    .entry(table)
                    .or_default()
                    .insert(effect as EffectId);
            }
            Ok(())
        })?;

        Ok(data)
    }
}

/// Error al cargar los datos del juego.
#[derive(Debug)]
pub enum DataError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Csv {
        path: String,
        source: csv::Error,
    },
    MissingColumn {
        path: String,
        column: String,
    },
    BadNumber {
        path: String,
        column: String,
        value: String,
    },
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io { path, source } => write!(f, "no se pudo abrir {path}: {source}"),
            DataError::Csv { path, source } => write!(f, "CSV inválido en {path}: {source}"),
            DataError::MissingColumn { path, column } => {
                write!(f, "a {path} le falta la columna '{column}'")
            }
            DataError::BadNumber {
                path,
                column,
                value,
            } => {
                write!(
                    f,
                    "en {path}, la columna '{column}' tiene un valor no numérico: '{value}'"
                )
            }
        }
    }
}

impl std::error::Error for DataError {}

/// Una fila de CSV con acceso por nombre de columna.
struct Row<'a> {
    path: &'a str,
    headers: &'a HashMap<String, usize>,
    record: &'a csv::StringRecord,
}

impl Row<'_> {
    fn text(&self, column: &str) -> &str {
        self.headers
            .get(column)
            .and_then(|&i| self.record.get(i))
            .unwrap_or("")
    }

    fn int(&self, column: &str) -> Result<i64, DataError> {
        let idx = *self
            .headers
            .get(column)
            .ok_or_else(|| DataError::MissingColumn {
                path: self.path.to_string(),
                column: column.to_string(),
            })?;
        let raw = self.record.get(idx).unwrap_or("").trim();
        raw.parse::<i64>().map_err(|_| DataError::BadNumber {
            path: self.path.to_string(),
            column: column.to_string(),
            value: raw.to_string(),
        })
    }
}

/// Recorre las filas de un CSV llamando a `f` con cada una.
fn for_each_row(
    path: &Path,
    mut f: impl FnMut(&Row<'_>) -> Result<(), DataError>,
) -> Result<(), DataError> {
    let path_str = path.display().to_string();
    let file = std::fs::File::open(path).map_err(|source| DataError::Io {
        path: path_str.clone(),
        source,
    })?;
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(file);
    let headers: HashMap<String, usize> = reader
        .headers()
        .map_err(|source| DataError::Csv {
            path: path_str.clone(),
            source,
        })?
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim_start_matches('\u{feff}').to_string(), i))
        .collect();

    for record in reader.records() {
        let record = record.map_err(|source| DataError::Csv {
            path: path_str.clone(),
            source,
        })?;
        f(&Row {
            path: &path_str,
            headers: &headers,
            record: &record,
        })?;
    }
    Ok(())
}
