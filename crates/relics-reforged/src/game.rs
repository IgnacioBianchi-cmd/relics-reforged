//! Acceso a los datos vivos del juego. Lectura, y por ahora una sola escritura experimental:
//! los efectos, maldiciones y SpEffects de una reliquia (`write_slots`).
//!
//! Objeto de ítem de una reliquia (0x100 bytes): +0x00 vtable, +0x08 registro de 80 bytes
//! (igual al del save), +0x5C lista de hasta 18 SpEffects, +0xA4 cantidad de SpEffects.
//!
//! Direcciones relevadas con Cheat Engine para `nightreign.exe` 1.3.3.0 (ver `docs/relevamiento.md`):
//! - murk: `[[nightreign.exe + 0x3C078D0] + 0x8] + 0xD0`
//! - ítem `i`: `objeto = [[nightreign.exe + 0x3C131C8] + 0x8 + i*8]`, registro en `objeto + 0x8`
//!   (mismo layout de 80 bytes que en el save).

use relic_rules::EMPTY;

use crate::win::{pe_info, read, write};

/// `TimeDateStamp` de la cabecera PE de `nightreign.exe` 1.3.3.0.
pub const SUPPORTED_TIMESTAMP: u32 = 0x69E0_DC2A;

const GAME_DATA_MAN_RVA: usize = 0x3C0_78D0;
const PLAYER_GAME_DATA: usize = 0x8;
const MURK: usize = 0xD0;

const GA_ITEM_RVA: usize = 0x3C1_31C8;
const ITEM_TABLE: usize = 0x8;
const RECORD_IN_OBJECT: usize = 0x8;
const EFFECTS_IN_RECORD: usize = 0x10;
const CURSES_IN_RECORD: usize = 0x38;
/// Lista de SpEffects calculada por el juego al cargar (no está en el save) y su cantidad.
const SP_LIST_IN_OBJECT: usize = 0x5C;
const SP_COUNT_IN_OBJECT: usize = 0xA4;
const MAX_SP_EFFECTS: usize = 18;

/// Función del juego que quita N unidades de un ítem por su handle (ver `docs/relevamiento.md`).
const REMOVE_ITEM_BY_HANDLE_RVA: usize = 0x7E_A470;
const REMOVE_ITEM_BY_HANDLE_PROLOGUE: [u8; 16] = [
    0x48, 0x89, 0x5C, 0x24, 0x08, 0x48, 0x89, 0x7C, 0x24, 0x18, 0x55, 0x48, 0x8D, 0x6C, 0x24, 0xA0,
];
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

    pub fn base(&self) -> usize {
        self.base
    }

    /// Dirección del murk en memoria.
    pub fn murk_addr(&self) -> Option<usize> {
        let game_data_man: usize = read(self.base + GAME_DATA_MAN_RVA)?;
        let player_game_data: usize = read(game_data_man + PLAYER_GAME_DATA)?;
        (player_game_data != 0).then_some(player_game_data + MURK)
    }

    pub fn murk(&self) -> Option<u32> {
        read(self.murk_addr()?)
    }

    fn item_table(&self) -> Option<usize> {
        read::<usize>(self.base + GA_ITEM_RVA).filter(|&t| t != 0)
    }

    /// Lee la entrada `index` de la tabla de ítems si es una reliquia con forma válida.
    /// Devuelve la dirección del objeto y la reliquia.
    fn relic_at(&self, table: usize, index: usize) -> Result<Option<(usize, LiveRelic)>, ()> {
        let Some(object) = read::<usize>(table + ITEM_TABLE + index * 8) else {
            return Err(()); // nos fuimos de la memoria legible
        };
        if object == 0 {
            return Ok(None);
        }
        let Some(rec) = read::<[u32; 20]>(object + RECORD_IN_OBJECT) else {
            return Ok(None);
        };
        let handle = rec[0];
        let is_relic_handle = handle >> 24 == 0xC0 && (handle & 0xFFFF) as usize == index;
        let item_ok = rec[1] == rec[2] && rec[1] & 0x8000_0000 != 0 && rec[3] == EMPTY;
        if !is_relic_handle || !item_ok {
            return Ok(None);
        }
        Ok(Some((
            object,
            LiveRelic {
                index,
                handle,
                item_id: rec[1] & 0x7FFF_FFFF,
                effects: [rec[4], rec[5], rec[6], rec[7]],
                curses: [rec[14], rec[15], rec[16], rec[17]],
            },
        )))
    }

    /// Busca una reliquia por su handle (`0xC08000NN`): dirección del objeto y datos.
    pub fn relic_by_handle(&self, handle: u32) -> Option<(usize, LiveRelic)> {
        let table = self.item_table()?;
        let index = (handle & 0xFFFF) as usize;
        if index >= MAX_ITEMS {
            return None;
        }
        self.relic_at(table, index)
            .ok()
            .flatten()
            .filter(|(_, r)| r.handle == handle)
    }

    /// Lista de SpEffects que el juego tiene calculada para la reliquia (ver `Relic::sp_effects`).
    pub fn sp_effects(&self, handle: u32) -> Option<Vec<u32>> {
        let (object, _) = self.relic_by_handle(handle)?;
        let count: u32 = read(object + SP_COUNT_IN_OBJECT)?;
        let list: [u32; MAX_SP_EFFECTS] = read(object + SP_LIST_IN_OBJECT)?;
        (count as usize <= MAX_SP_EFFECTS).then(|| list[..count as usize].to_vec())
    }

    /// Reescribe efectos, maldiciones y la lista de SpEffects de una reliquia en memoria.
    /// No toca ningún otro campo del objeto.
    pub fn write_slots(
        &self,
        handle: u32,
        effects: [u32; 4],
        curses: [u32; 4],
        sp_effects: &[u32],
    ) -> bool {
        let Some((object, _)) = self.relic_by_handle(handle) else {
            return false;
        };
        if sp_effects.len() > MAX_SP_EFFECTS {
            return false;
        }
        let mut list = [EMPTY; MAX_SP_EFFECTS];
        list[..sp_effects.len()].copy_from_slice(sp_effects);
        let rec = object + RECORD_IN_OBJECT;
        write(rec + EFFECTS_IN_RECORD, effects)
            && write(rec + CURSES_IN_RECORD, curses)
            && write(object + SP_LIST_IN_OBJECT, list)
            && write(object + SP_COUNT_IN_OBJECT, sp_effects.len() as u32)
    }

    /// Quita `quantity` unidades del ítem con ese handle usando la función del propio juego
    /// (la misma cadena que usa la venta: desequipa, saca del inventario y libera el objeto).
    /// No toca el murk. Devuelve `Err` si el código del juego no es el esperado.
    ///
    /// # Safety
    /// Llama código del juego desde nuestro hilo: el inventario no debería estar en uso
    /// (por ejemplo, con el menú de reliquias abierto) mientras tanto.
    pub unsafe fn remove_item(&self, handle: u32, quantity: i32) -> Result<bool, String> {
        let addr = self.base + REMOVE_ITEM_BY_HANDLE_RVA;
        let prologue: [u8; 16] = read(addr).ok_or("no puedo leer el código")?;
        if prologue != REMOVE_ITEM_BY_HANDLE_PROLOGUE {
            return Err(format!(
                "el código en nr+{REMOVE_ITEM_BY_HANDLE_RVA:X} no es el esperado"
            ));
        }
        // nr+7EA470: bool RemoveItemByHandle(const u32* handle, i32 quantity)
        let f: unsafe extern "win64" fn(*const u32, i32) -> u8 = std::mem::transmute(addr);
        let handle_cell = handle;
        Ok(f(&handle_cell, quantity) != 0)
    }

    /// Recorre la tabla de ítems y devuelve las reliquias, validando la forma de cada registro.
    pub fn relics(&self) -> Vec<LiveRelic> {
        let Some(table) = self.item_table() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for index in 0..MAX_ITEMS {
            match self.relic_at(table, index) {
                Err(()) => break,
                Ok(Some((_, relic))) => out.push(relic),
                Ok(None) => {}
            }
        }
        out
    }
}
