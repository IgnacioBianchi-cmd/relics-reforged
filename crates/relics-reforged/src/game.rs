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

use relic_rules::lang::tr;
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

/// EquipGameData dentro de PlayerGameData, y el inventario dentro de EquipGameData.
const EQUIP_GAME_DATA: usize = 0x1D8;
const INVENTORY_IN_EQUIP: usize = 0x1C8;
/// Inventario: índices `< [+0x1C]` en el array de `[+0x40]`, el resto en el de `[+0x30]`.
const INV_SPLIT: usize = 0x1C;
const INV_ARRAY_HIGH: usize = 0x30;
const INV_ARRAY_LOW: usize = 0x40;
const INV_LAST_INDEX: usize = 0x70;
const INV_ENTRY_SIZE: usize = 20;

/// Slots de reliquia del recipiente equipado ahora (6 handles). Solo se mira esto de
/// PlayerGameData: más adelante (+0x8FC..) hay una lista con todas las reliquias, que no
/// significa que estén en uso.
const EQUIPPED_SLOTS: usize = 0x2F4;
/// Tipo de reliquia de cada slot equipado (-1 = vacío).
const EQUIPPED_TYPES: usize = 0x374;
/// Puntero al contenedor de recipientes y presets: {asignador, cabeza del `std::map` de
/// recipientes, cantidad} y {asignador, ?, cantidad de presets}, seguidos de los presets.
const RELIC_SETS_PTR: usize = 0x6F8;
const ALLOCATOR_VTABLE_RVA: usize = 0x39B_8008;
/// Presets: cantidad en `sets+0x28`, registros de 0x50 bytes desde `sets+0x30`. Cada uno: marcas
/// (+0x00), nombre en UTF-16 (+0x04, hasta 20 caracteres), recipiente (+0x2C, -1 = preset vacío),
/// 6 handles (+0x30, 0 = slot vacío) y FILETIME (+0x48). Relevado el 2026-11-14.
const PRESET_COUNT: usize = 0x28;
const PRESETS: usize = 0x30;
const PRESET_SIZE: usize = 0x50;
const PRESET_NAME: usize = 0x04;
const PRESET_NAME_LEN: usize = 20;
const PRESET_HANDLES: usize = 0x30;

/// `RemoveItem(EquipGameData*, índice, delta, flag)`, la que usa la venta (ver `docs/relevamiento.md`).
const REMOVE_ITEM_RVA: usize = 0x1F_06B0;
const REMOVE_ITEM_PROLOGUE: [u8; 16] = [
    0x48, 0x89, 0x5C, 0x24, 0x10, 0x44, 0x89, 0x44, 0x24, 0x18, 0x55, 0x56, 0x57, 0x41, 0x54, 0x41,
];
/// Cota superior de entradas a recorrer (Elden Ring usa 5120; sobra margen).
const MAX_ITEMS: usize = 0x2000;

/// ¿La entrada de inventario `[handle, item, cantidad, orden, marcas]` es favorita? Es el byte
/// bajo de las marcas; los bytes altos traen otro dato que cambia entre sesiones.
pub fn is_favorite(entry: &[u32; 5]) -> bool {
    entry[4] & 0xFF != 0
}

/// Un lugar de los datos del jugador que menciona una reliquia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefPlace {
    /// Slot (0..6) del recipiente equipado ahora.
    Equipped(usize),
    /// Preset guardado: índice (0..) y nombre.
    Preset(usize, String),
    /// Lo que recuerda un recipiente, por su ID interno.
    Vessel(u32),
}

impl RefPlace {
    /// Para el jugador: sin IDs internos.
    pub fn label(&self) -> String {
        match self {
            RefPlace::Equipped(_) => tr("equipada", "equipped").into(),
            RefPlace::Preset(i, name) if name.trim().is_empty() => format!("preset {}", i + 1),
            RefPlace::Preset(_, name) => format!("preset \"{name}\""),
            RefPlace::Vessel(_) => tr("recipiente", "vessel").into(),
        }
    }

    /// Con los IDs internos, para el log y los comandos.
    pub fn detail(&self) -> String {
        match self {
            RefPlace::Equipped(slot) => {
                format!("{} (slot {})", tr("equipada", "equipped"), slot + 1)
            }
            RefPlace::Preset(i, name) => format!("preset {} \"{name}\"", i + 1),
            RefPlace::Vessel(id) => format!("{} {id:#X}", tr("recipiente", "vessel")),
        }
    }
}

/// Una mención de una reliquia: qué handle, dónde y la dirección donde está escrito.
#[derive(Debug, Clone)]
pub struct RelicRef {
    pub handle: u32,
    pub place: RefPlace,
    pub addr: usize,
}

/// Una reliquia leída de la memoria del juego.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRelic {
    pub index: usize,
    pub handle: u32,
    pub item_id: u32,
    pub effects: [u32; 4],
    pub curses: [u32; 4],
}

#[derive(Clone, Copy)]
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

    /// Cambia el tipo de una reliquia (fila de `EquipParamAntique`) en los tres lugares donde está:
    /// el registro del objeto (+4 y +8, como en el save) y su entrada de inventario. Se usa al
    /// pasar una reliquia a la versión nueva de su tipo (`relic_rules::newer_twin`).
    pub fn write_item_id(&self, handle: u32, item_id: u32) -> bool {
        let Some((object, _)) = self.relic_by_handle(handle) else {
            return false;
        };
        let Some(inv) = self.equip_game_data().map(|e| e + INVENTORY_IN_EQUIP) else {
            return false;
        };
        let Some(entry) = self
            .inventory_entry(handle)
            .and_then(|(index, _)| self.inventory_entry_addr(inv, index))
        else {
            return false;
        };
        let raw = item_id | 0x8000_0000;
        let rec = object + RECORD_IN_OBJECT;
        write(rec + 4, raw) && write(rec + 8, raw) && write(entry + 4, raw)
    }

    /// Dirección de EquipGameData (PlayerGameData + 0x1D8), lo que recibe `RemoveItem`.
    fn equip_game_data(&self) -> Option<usize> {
        let game_data_man: usize = read(self.base + GAME_DATA_MAN_RVA)?;
        let player_game_data: usize = read(game_data_man + PLAYER_GAME_DATA)?;
        (player_game_data != 0).then_some(player_game_data + EQUIP_GAME_DATA)
    }

    /// Dirección de la entrada `index` del inventario (20 bytes: handle, item, cantidad, ...).
    fn inventory_entry_addr(&self, inv: usize, index: u32) -> Option<usize> {
        let split: u32 = read(inv + INV_SPLIT)?;
        let (array, i) = if index < split {
            (read::<usize>(inv + INV_ARRAY_LOW)?, index)
        } else {
            (read::<usize>(inv + INV_ARRAY_HIGH)?, index - split)
        };
        (array != 0).then_some(array + i as usize * INV_ENTRY_SIZE)
    }

    /// ¿Está abierto el menú de Relic Rites? (ver `relic_menu`). Ojo: `nr+46F92F8`, que en un
    /// primer relevamiento parecía una marca, cambia frame a frame y no sirve.
    pub fn relic_rites_open(&self) -> bool {
        crate::relic_menu::is_open()
    }

    pub fn game_data_man(&self) -> Option<usize> {
        read::<usize>(self.base + GAME_DATA_MAN_RVA).filter(|&p| p != 0)
    }

    pub fn player_game_data(&self) -> Option<usize> {
        let game_data_man: usize = read(self.base + GAME_DATA_MAN_RVA)?;
        read::<usize>(game_data_man + PLAYER_GAME_DATA).filter(|&p| p != 0)
    }

    /// Todas las entradas del inventario: (índice, [handle, item, cantidad, orden, flag]).
    pub fn inventory_entries(&self) -> Vec<(u32, [u32; 5])> {
        let Some(inv) = self.equip_game_data().map(|e| e + INVENTORY_IN_EQUIP) else {
            return Vec::new();
        };
        let Some(last) = read::<u32>(inv + INV_LAST_INDEX).filter(|&l| l <= 0x4000) else {
            return Vec::new();
        };
        (0..=last)
            .filter_map(|index| {
                let entry: [u32; 5] = read(self.inventory_entry_addr(inv, index)?)?;
                Some((index, entry))
            })
            .collect()
    }

    /// Dónde aparece mencionado cada handle de reliquia (`0xC080xxxx`) en los datos del jugador:
    /// PlayerGameData (slots equipados en +0x2F4), los presets y el árbol de recipientes
    /// (`std::map`), estos dos en el contenedor de `[pgd+0x6F8]`. Devuelve cada aparición con su
    /// dirección, o `Err` si las estructuras no están donde se esperan (en ese caso no se puede
    /// garantizar nada y no hay que forjar).
    pub fn relic_reference_list(&self) -> Result<Vec<RelicRef>, String> {
        let pgd = self
            .player_game_data()
            .ok_or("no encuentro PlayerGameData")?;
        // Contenedor de recipientes y presets: lo apunta pgd+0x6F8. Sus dos contenedores
        // internos empiezan con el puntero al asignador del juego: si no, no es lo que creemos.
        let sets: usize = read(pgd + RELIC_SETS_PTR).ok_or("no puedo leer PlayerGameData")?;
        let alloc = self.base + ALLOCATOR_VTABLE_RVA;
        let a0: usize = read(sets).unwrap_or(0);
        let a1: usize = read(sets + 0x18).unwrap_or(0);
        if sets == 0 || a0 != alloc || a1 != alloc {
            return Err(format!(
                "los recipientes y presets no están donde se espera ([pgd+{RELIC_SETS_PTR:X}] = {sets:#X})"
            ));
        }
        let mut out = Vec::new();
        // Los 6 handles que empiezan en `base`.
        let mut scan = |base: usize, place: &dyn Fn(usize) -> RefPlace| -> Result<(), String> {
            let handles: [u32; 6] =
                read(base).ok_or_else(|| format!("no puedo leer {base:#X}"))?;
            for (slot, h) in handles.into_iter().enumerate() {
                if h >> 16 == 0xC080 {
                    out.push(RelicRef {
                        handle: h,
                        place: place(slot),
                        addr: base + slot * 4,
                    });
                }
            }
            Ok(())
        };
        scan(pgd + EQUIPPED_SLOTS, &RefPlace::Equipped)?;
        let presets: usize = read(sets + PRESET_COUNT).ok_or("no puedo leer los presets")?;
        if presets > 0x100 {
            return Err(format!("cantidad de presets inválida ({presets})"));
        }
        for i in 0..presets {
            let preset = sets + PRESETS + i * PRESET_SIZE;
            let name: [u16; PRESET_NAME_LEN] =
                read(preset + PRESET_NAME).ok_or("no puedo leer los presets")?;
            let len = name.iter().position(|&c| c == 0).unwrap_or(PRESET_NAME_LEN);
            let name = String::from_utf16_lossy(&name[..len]);
            scan(preset + PRESET_HANDLES, &|_| RefPlace::Preset(i, name.clone()))?;
        }
        // Árbol rojo-negro de MSVC: nodo = izquierda, padre, derecha, color, esNulo (+0x19),
        // clave = ID del recipiente (+0x1C), 6 handles desde +0x24.
        let head: usize = read(sets + 8).ok_or("no puedo leer el árbol")?;
        let count: usize = read(sets + 0x10).ok_or("no puedo leer el árbol")?;
        if head == 0 || count > 0x1000 {
            return Err(format!(
                "árbol de recipientes inválido ({head:#X}, {count})"
            ));
        }
        let root: usize = read(head + 8).ok_or("no puedo leer el árbol")?;
        let mut stack = vec![root];
        let mut visited = 0;
        while let Some(node) = stack.pop() {
            let is_nil: u8 = read(node + 0x19).ok_or("nodo ilegible")?;
            if node == head || is_nil != 0 {
                continue;
            }
            visited += 1;
            if visited > count {
                return Err("el árbol de recipientes tiene más nodos de los que dice".into());
            }
            let vessel: u32 = read(node + 0x1C).ok_or("nodo ilegible")?;
            scan(node + 0x24, &|_| RefPlace::Vessel(vessel))?;
            stack.push(read(node).ok_or("nodo ilegible")?);
            stack.push(read(node + 0x10).ok_or("nodo ilegible")?);
        }
        if visited != count {
            return Err(format!(
                "recorrí {visited} recipientes de {count}: el árbol no es lo que parece"
            ));
        }
        Ok(out)
    }

    /// Como [`relic_reference_list`](Self::relic_reference_list), con el lugar en texto (con
    /// los IDs internos, para el log).
    pub fn relic_references(&self) -> Result<Vec<(u32, String)>, String> {
        Ok(self
            .relic_reference_list()?
            .into_iter()
            .map(|r| (r.handle, r.place.detail()))
            .collect())
    }

    /// Dónde está en uso la reliquia (sin repetir lugares, como los ve el jugador).
    pub fn places_in_use(&self, handle: u32) -> Result<Vec<String>, String> {
        let mut places: Vec<String> = Vec::new();
        for r in self.relic_reference_list()? {
            let label = r.place.label();
            if r.handle == handle && !places.contains(&label) {
                places.push(label);
            }
        }
        Ok(places)
    }

    /// Si la reliquia no se puede sacrificar: favorita, equipada, recordada por un recipiente o
    /// guardada en un preset. Devuelve el motivo. Si no se puede verificar, también la da por
    /// bloqueada.
    pub fn lock_reason(&self, handle: u32) -> Option<String> {
        if let Some((_, entry)) = self.inventory_entry(handle) {
            if is_favorite(&entry) {
                return Some(tr("es favorita", "is a favorite").into());
            }
        }
        match self.places_in_use(handle) {
            Err(e) => Some(format!(
                "{}: {e}",
                tr(
                    "no se pudo verificar si está en uso",
                    "couldn't check whether it is in use"
                )
            )),
            Ok(places) if places.is_empty() => None,
            Ok(places) => Some(format!(
                "{}: {}",
                tr("está en uso", "is in use"),
                places.join(", ")
            )),
        }
    }

    /// Saca la reliquia de todos los lugares donde está mencionada (slots equipados, presets y
    /// recipientes) y deja esos slots vacíos como los deja el juego: handle 0 y, en los
    /// equipados, el tipo de reliquia en -1. Se usa con la donante después de borrarla, para que
    /// ningún preset ni recipiente apunte a un handle que el juego puede volver a usar.
    /// Devuelve los lugares de donde se sacó.
    pub fn clear_references(&self, handle: u32) -> Result<Vec<RefPlace>, String> {
        let pgd = self
            .player_game_data()
            .ok_or("no encuentro PlayerGameData")?;
        let mut cleared = Vec::new();
        for r in self.relic_reference_list()? {
            if r.handle != handle {
                continue;
            }
            let ok = read::<u32>(r.addr) == Some(handle)
                && write(r.addr, 0u32)
                && match r.place {
                    RefPlace::Equipped(slot) => write(pgd + EQUIPPED_TYPES + slot * 4, EMPTY),
                    _ => true,
                };
            if !ok {
                return Err(format!(
                    "{} {}",
                    tr("no pude sacar la reliquia de", "couldn't remove the relic from"),
                    r.place.detail()
                ));
            }
            cleared.push(r.place);
        }
        Ok(cleared)
    }

    /// Busca la entrada de inventario de un handle: (índice, [handle, item, cantidad, x, y]).
    pub fn inventory_entry(&self, handle: u32) -> Option<(u32, [u32; 5])> {
        let inv = self.equip_game_data()? + INVENTORY_IN_EQUIP;
        let last: u32 = read(inv + INV_LAST_INDEX)?;
        if last > 0x4000 {
            return None; // no parece un inventario
        }
        (0..=last).find_map(|index| {
            let entry: [u32; 5] = read(self.inventory_entry_addr(inv, index)?)?;
            (entry[0] == handle).then_some((index, entry))
        })
    }

    /// Quita una unidad del ítem con ese handle llamando a `RemoveItem` del propio juego, igual que
    /// la venta: desequipa, saca del inventario y libera el objeto. No toca el murk.
    /// Devuelve lo que devuelve el juego (la cantidad que queda).
    ///
    /// # Safety
    /// Llama código del juego: tiene que correr en el hilo principal (ver `mainthread::run`).
    pub unsafe fn remove_item(&self, handle: u32) -> Result<i32, String> {
        let addr = self.base + REMOVE_ITEM_RVA;
        let prologue: [u8; 16] = read(addr).ok_or("no puedo leer el código")?;
        if prologue != REMOVE_ITEM_PROLOGUE {
            return Err(format!(
                "el código en nr+{REMOVE_ITEM_RVA:X} no es el esperado"
            ));
        }
        let equip = self.equip_game_data().ok_or("no encuentro EquipGameData")?;
        let (index, _) = self
            .inventory_entry(handle)
            .ok_or("el handle no está en el inventario")?;
        // nr+1F06B0: i32 RemoveItem(EquipGameData*, u32 index, i32 delta, bool flag)
        let f: unsafe extern "win64" fn(usize, u32, i32, u8) -> i32 = std::mem::transmute(addr);
        Ok(f(equip, index, -1, 0))
    }

    /// Verifica que el código de las funciones del juego que usamos sea el esperado.
    pub fn check_game_functions(&self) -> Result<(), String> {
        let actual: [u8; 16] =
            read(self.base + REMOVE_ITEM_RVA).ok_or("no puedo leer el código")?;
        if actual != REMOVE_ITEM_PROLOGUE {
            return Err(format!(
                "el código en nr+{REMOVE_ITEM_RVA:X} no es el esperado"
            ));
        }
        Ok(())
    }

    // NO llamar a nr+6AF930: no es "guardar ya" sino "foto en memoria / restaurar foto". Si ya
    // hay una foto (GameMan+0xDA8), la carga encima de la partida (probado el 2026-11-04: dejó
    // un personaje por defecto con 9.999.999 de murk). Ver docs/relevamiento.md.

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
