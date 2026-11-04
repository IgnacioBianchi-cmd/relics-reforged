//! Acceso de SOLO LECTURA a los datos vivos del juego.
//!
//! Direcciones relevadas con Cheat Engine para `nightreign.exe` 1.3.3.0 (ver `docs/relevamiento.md`):
//! - murk: `[[nightreign.exe + 0x3C078D0] + 0x8] + 0xD0`
//! - ítem `i`: `objeto = [[nightreign.exe + 0x3C131C8] + 0x8 + i*8]`, registro en `objeto + 0x8`
//!   (mismo layout de 80 bytes que en el save).

use relic_rules::EMPTY;

use crate::win::{pe_info, read};

/// `TimeDateStamp` de la cabecera PE de `nightreign.exe` 1.3.3.0.
pub const SUPPORTED_TIMESTAMP: u32 = 0x69E0_DC2A;

const GAME_DATA_MAN_RVA: usize = 0x3C0_78D0;
const PLAYER_GAME_DATA: usize = 0x8;
const MURK: usize = 0xD0;

const GA_ITEM_RVA: usize = 0x3C1_31C8;
const ITEM_TABLE: usize = 0x8;
const RECORD_IN_OBJECT: usize = 0x8;
/// Cota superior de entradas a recorrer (Elden Ring usa 5120; sobra margen).
const MAX_ITEMS: usize = 0x2000;

/// Una reliquia leída de la memoria del juego.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRelic {
    pub index: usize,
    pub handle: u32,
    pub item_id: u32,
    pub effects: [u32; 4],
    pub curses: [u32; 4],
}

pub struct Game {
    base: usize,
}

impl Game {
    /// Devuelve `Err(timestamp)` si el ejecutable no es la versión soportada.
    pub fn attach(base: usize) -> Result<Self, Option<u32>> {
        match pe_info(base) {
            Some((ts, _)) if ts == SUPPORTED_TIMESTAMP => Ok(Game { base }),
            Some((ts, _)) => Err(Some(ts)),
            None => Err(None),
        }
    }

    pub fn murk(&self) -> Option<u32> {
        let game_data_man: usize = read(self.base + GAME_DATA_MAN_RVA)?;
        let player_game_data: usize = read(game_data_man + PLAYER_GAME_DATA)?;
        read(player_game_data + MURK)
    }

    /// Recorre la tabla de ítems y devuelve las reliquias, validando la forma de cada registro.
    pub fn relics(&self) -> Vec<LiveRelic> {
        let Some(table) = read::<usize>(self.base + GA_ITEM_RVA) else {
            return Vec::new();
        };
        if table == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for index in 0..MAX_ITEMS {
            let Some(object) = read::<usize>(table + ITEM_TABLE + index * 8) else {
                break; // nos fuimos de la memoria legible
            };
            if object == 0 {
                continue;
            }
            let Some(rec) = read::<[u32; 20]>(object + RECORD_IN_OBJECT) else {
                continue;
            };
            let handle = rec[0];
            let is_relic_handle = handle >> 24 == 0xC0 && (handle & 0xFFFF) as usize == index;
            let item_ok = rec[1] == rec[2] && rec[1] & 0x8000_0000 != 0 && rec[3] == EMPTY;
            if !is_relic_handle || !item_ok {
                continue;
            }
            out.push(LiveRelic {
                index,
                handle,
                item_id: rec[1] & 0x7FFF_FFFF,
                effects: [rec[4], rec[5], rec[6], rec[7]],
                curses: [rec[14], rec[15], rec[16], rec[17]],
            });
        }
        out
    }
}
