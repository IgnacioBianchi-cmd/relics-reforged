//! Datos de juego sintéticos para los tests unitarios.
//!
//! - Tipo 1: normal grande, 3 slots con la tabla 100.
//! - Tipo 2: Deep mediana, slot 1 potenciado (tabla 200 + maldición 300), slot 2 normal (tabla 210).
//! - Tipo 3: normal chica, 1 slot con la tabla 110.
//! - Tipo 9: fija, 1 slot con la tabla 900, que tiene un solo efecto.
//!
//! Los efectos 12 y 13 comparten `compatibilityId` (como Vigor +1 y Vigor +2).

use crate::data::{EffectInfo, GameData, RelicType};

pub fn sample_data() -> GameData {
    let mut data = GameData::default();

    let effect = |id: u32, compat: i32, debuff: bool| EffectInfo {
        id,
        name: format!("efecto {id}"),
        compatibility_id: compat,
        sort_priority: 100,
        is_debuff: debuff,
        passive_sp_effects: vec![id + 1000],
        text_id: id as i32,
        icon_id: -1,
        filter_id: -1,
    };
    for e in [
        effect(10, 100, false),
        effect(11, 101, false),
        effect(12, 500, false),
        effect(13, 500, false),
        effect(14, 102, false),
        effect(15, 103, false),
        effect(16, 104, false),
        effect(20, 200, false),
        effect(21, 201, false),
        effect(22, 202, false),
        effect(23, 203, false),
        effect(30, -1, true),
        effect(31, -1, true),
    ] {
        data.effects.insert(e.id, e);
    }

    let normal_pool = [10, 11, 12, 13, 14, 15, 16];
    for (table, effects) in [
        (100, &normal_pool[..]),
        (110, &normal_pool[..]),
        (111, &[10, 11, 12, 13, 14, 15, 16, 20][..]),
        (200, &[20, 21][..]),
        (210, &[22, 23][..]),
        (300, &[30, 31][..]),
        (900, &[10][..]),
    ] {
        data.tables.insert(table, effects.iter().copied().collect());
    }

    let relic = |id: u32, deep: bool, slots: [i32; 3], curses: [i32; 3]| RelicType {
        id,
        name: format!("tipo {id}"),
        is_deep: deep,
        is_salable: true,
        color: 0,
        sort_group: 255,
        slot_tables: slots,
        curse_tables: curses,
        icon_id: -1,
    };
    for rt in [
        relic(1, false, [100, 100, 100], [-1, -1, -1]),
        relic(2, true, [200, 210, -1], [300, -1, -1]),
        relic(3, false, [110, -1, -1], [-1, -1, -1]),
        relic(9, false, [900, -1, -1], [-1, -1, -1]),
        // Versión nueva del tipo 3: su tabla suma el efecto 20.
        relic(103, false, [111, -1, -1], [-1, -1, -1]),
    ] {
        data.relic_types.insert(rt.id, rt);
    }
    // Como el 3, pero del Small Jar Bazaar (única).
    let mut unique = relic(1_005_000, false, [110, -1, -1], [-1, -1, -1]);
    unique.sort_group = 250;
    data.relic_types.insert(unique.id, unique);

    data
}
