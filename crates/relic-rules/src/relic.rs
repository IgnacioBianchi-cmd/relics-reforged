//! Modelo de una reliquia del inventario y su clasificación.

use crate::data::{EffectId, GameData, ItemId, EMPTY};

/// Un slot de efecto de una reliquia: el efecto y, si es Deep potenciado, su maldición.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Slot {
    pub effect: Option<EffectId>,
    pub curse: Option<EffectId>,
}

impl Slot {
    pub const EMPTY: Slot = Slot {
        effect: None,
        curse: None,
    };

    pub fn new(effect: EffectId, curse: Option<EffectId>) -> Self {
        Slot {
            effect: Some(effect),
            curse,
        }
    }
}

/// Una reliquia concreta: su tipo y sus 3 slots, en el orden en que los guarda el juego.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relic {
    pub item_id: ItemId,
    pub slots: [Slot; 3],
    /// Favorita o equipada en un preset: no se puede donar ni sacrificar.
    pub locked: bool,
}

impl Relic {
    pub fn new(item_id: ItemId, slots: [Slot; 3]) -> Self {
        Relic {
            item_id,
            slots,
            locked: false,
        }
    }

    /// Construye una reliquia desde los campos crudos del registro de 80 bytes
    /// (4 efectos en +0x10 y 4 maldiciones en +0x38; el 4º slot no se usa).
    pub fn from_raw(item_id: ItemId, effects: [u32; 4], curses: [u32; 4]) -> Self {
        let opt = |v: u32| (v != EMPTY).then_some(v);
        let mut slots = [Slot::EMPTY; 3];
        for (i, slot) in slots.iter_mut().enumerate() {
            *slot = Slot {
                effect: opt(effects[i]),
                curse: opt(curses[i]),
            };
        }
        Relic::new(item_id, slots)
    }

    /// Devuelve los campos crudos (efectos, maldiciones) para escribir el registro.
    pub fn to_raw(&self) -> ([u32; 4], [u32; 4]) {
        let mut effects = [EMPTY; 4];
        let mut curses = [EMPTY; 4];
        for (i, slot) in self.slots.iter().enumerate() {
            effects[i] = slot.effect.unwrap_or(EMPTY);
            curses[i] = slot.curse.unwrap_or(EMPTY);
        }
        (effects, curses)
    }

    /// Efectos presentes, en orden.
    pub fn effects(&self) -> impl Iterator<Item = EffectId> + '_ {
        self.slots.iter().filter_map(|s| s.effect)
    }

    pub fn effect_count(&self) -> usize {
        self.effects().count()
    }

    /// Lista de SpEffects que el juego guarda en memoria junto a la reliquia (objeto + 0x5C,
    /// con la cantidad en + 0xA4): los `passiveSpEffectId` de cada efecto en orden de slot y
    /// después los de cada maldición, sin quitar repetidos. Verificado contra el juego.
    /// Devuelve `None` si algún efecto no está en los params.
    pub fn sp_effects(&self, data: &GameData) -> Option<Vec<u32>> {
        let curses = self.slots.iter().filter_map(|s| s.curse);
        let mut out = Vec::new();
        for id in self.effects().chain(curses) {
            out.extend_from_slice(&data.effects.get(&id)?.passive_sp_effects);
        }
        Some(out)
    }
}

/// Qué tipo de reliquia es, según sus tablas de roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelicKind {
    /// Reliquia aleatoria: la única que el mod deja donar, recibir o sacrificar.
    /// `size` = cantidad de slots con tabla (1 = chica, 2 = mediana, 3 = grande).
    Random { deep: bool, size: usize },
    /// Reliquia de efectos fijos (Bazaar, jefes, quests): cada tabla tiene un solo efecto.
    Fixed,
    /// El item_id no figura en `EquipParamAntique`.
    Unknown,
}

impl RelicKind {
    pub fn is_random(self) -> bool {
        matches!(self, RelicKind::Random { .. })
    }
}

/// Clasifica un tipo de reliquia. Es aleatoria si tiene al menos un slot con tabla y todas
/// sus tablas de slot ofrecen más de un efecto posible.
pub fn relic_kind(data: &GameData, item_id: ItemId) -> RelicKind {
    let Some(rt) = data.relic_types.get(&item_id) else {
        return RelicKind::Unknown;
    };
    let used: Vec<_> = rt.slot_tables.iter().copied().filter(|&t| t >= 0).collect();
    if used.is_empty() || used.iter().any(|&t| data.table_len(t) <= 1) {
        return RelicKind::Fixed;
    }
    RelicKind::Random {
        deep: rt.is_deep,
        size: used.len(),
    }
}
