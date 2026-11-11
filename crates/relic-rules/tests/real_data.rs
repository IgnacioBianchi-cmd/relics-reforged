//! Tests contra datos reales: los CSV de Smithbox (`data/export/`) y las reliquias de un save
//! (`data/save/relics.json`, generado con `tools/relic_survey.py`).
//!
//! Esos archivos no van en git (son del juego y del jugador). Si faltan, los tests avisan y pasan.

use std::path::PathBuf;

use relic_rules::*;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct SaveRelic {
    kind_label: String,
    name: String,
    relic: Relic,
}

fn load() -> Option<(GameData, Vec<SaveRelic>)> {
    let root = repo_root();
    let export = root.join("data/export");
    let save = root.join("data/save/relics.json");
    if !export.join("AttachEffectParam.csv").exists() || !save.exists() {
        eprintln!(
            "(sin datos reales en {}: se saltea el test)",
            root.display()
        );
        return None;
    }
    let data = GameData::from_csv_dir(&export).expect("cargar CSV");
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&save).unwrap()).expect("leer relics.json");
    let arr = |v: &serde_json::Value| -> [u32; 4] {
        let a = v.as_array().unwrap();
        std::array::from_fn(|i| a[i].as_u64().unwrap() as u32)
    };
    let relics = raw
        .as_array()
        .unwrap()
        .iter()
        .map(|r| SaveRelic {
            kind_label: r["type"].as_str().unwrap().to_string(),
            name: r["item_name"].as_str().unwrap().to_string(),
            relic: Relic::from_raw(
                r["item"].as_u64().unwrap() as u32,
                arr(&r["effects"]),
                arr(&r["curses"]),
            ),
        })
        .collect();
    Some((data, relics))
}

/// `regulation.bin` del juego instalado (ruta de esta PC; si no existe, el test se saltea).
const REGULATION: &str =
    r"B:\SteamLibrary\steamapps\common\ELDEN RING NIGHTREIGN\Game\regulation.bin";

/// Los datos leídos de `regulation.bin` tienen que ser idénticos a los de los CSV (salvo nombres).
#[test]
fn regulation_coincide_con_los_csv() {
    let Some((csv, _)) = load() else { return };
    let Ok(raw) = std::fs::read(REGULATION) else {
        eprintln!("(sin {REGULATION}: se saltea el test)");
        return;
    };
    let reg = GameData::from_regulation(&raw).expect("leer regulation.bin");

    assert_eq!(reg.effects.len(), csv.effects.len(), "cantidad de efectos");
    for (id, a) in &csv.effects {
        let b = reg
            .effects
            .get(id)
            .unwrap_or_else(|| panic!("falta el efecto {id}"));
        assert_eq!(
            a.compatibility_id, b.compatibility_id,
            "compatibilityId de {id}"
        );
        assert_eq!(a.sort_priority, b.sort_priority, "overrideEffectId de {id}");
        assert_eq!(a.is_debuff, b.is_debuff, "isDebuff de {id}");
        assert_eq!(
            a.passive_sp_effects, b.passive_sp_effects,
            "SpEffects de {id}"
        );
        assert_eq!(a.text_id, b.text_id, "attachTextId de {id}");
        assert_eq!(a.icon_id, b.icon_id, "statusIconId de {id}");
        assert_eq!(a.filter_id, b.filter_id, "attachFilterParamId de {id}");
    }

    assert_eq!(
        reg.relic_types.len(),
        csv.relic_types.len(),
        "cantidad de tipos"
    );
    for (id, a) in &csv.relic_types {
        let b = reg
            .relic_types
            .get(id)
            .unwrap_or_else(|| panic!("falta el tipo {id}"));
        assert_eq!(a.is_deep, b.is_deep, "isDeepRelic de {id}");
        assert_eq!(a.is_salable, b.is_salable, "isSalable de {id}");
        assert_eq!(a.color, b.color, "relicColor de {id}");
        assert_eq!(a.sort_group, b.sort_group, "sortGroupId de {id}");
        assert_eq!(a.slot_tables, b.slot_tables, "tablas de {id}");
        assert_eq!(a.curse_tables, b.curse_tables, "maldiciones de {id}");
        assert_eq!(a.icon_id, b.icon_id, "iconId de {id}");
    }

    assert_eq!(reg.tables, csv.tables, "tablas de efectos");

    // Versión nueva de los tipos: las 36 escenas 100-135 pasan a 200-235 (mismo color, tamaño y
    // tablas que incluyen las viejas); ningún otro tipo tiene.
    let mut twins: Vec<(u32, u32)> = reg
        .relic_types
        .keys()
        .filter_map(|&id| relic_rules::newer_twin(&reg, id).map(|t| (id, t)))
        .collect();
    twins.sort();
    assert_eq!(twins.len(), 36, "tipos con versión nueva: {twins:?}");
    assert!(
        twins
            .iter()
            .all(|&(a, b)| (100..=135).contains(&a) && b == a + 100),
        "{twins:?}"
    );
    // Filtro por efecto: cada familia usada por un efecto existe, y cada familia y grupo
    // apuntan a algo que existe.
    let f = &reg.effect_filters;
    assert_eq!(f.categories.len(), 4, "categorías del filtro");
    assert!(f.groups.len() > 50, "grupos del filtro: {}", f.groups.len());
    assert!(
        f.families.len() > 300,
        "familias del filtro: {}",
        f.families.len()
    );
    for g in &f.groups {
        assert!(
            f.categories.iter().any(|c| c.id == g.category),
            "categoría {} del grupo {}",
            g.category,
            g.id
        );
    }
    for fam in &f.families {
        assert!(
            f.groups.iter().any(|g| g.id == fam.group),
            "grupo {} de la familia {}",
            fam.group,
            fam.id
        );
    }
    // El juego tiene alguna referencia rota (6633200, "Discover Torches", apunta a una familia que
    // no existe): se toleran unas pocas.
    let missing: Vec<_> = reg
        .effects
        .values()
        .filter(|e| e.filter_id >= 0 && !f.families.iter().any(|fam| fam.id == e.filter_id))
        .map(|e| e.id)
        .collect();
    assert!(
        missing.len() <= 3,
        "efectos con familia inexistente: {missing:?}"
    );
}

#[test]
fn datos_cargan_completos() {
    let Some((data, _)) = load() else { return };
    assert!(data.effects.len() > 2000, "efectos: {}", data.effects.len());
    assert!(
        data.relic_types.len() > 1300,
        "tipos: {}",
        data.relic_types.len()
    );
    assert!(data.tables.len() > 1000, "tablas: {}", data.tables.len());
}

/// Toda reliquia aleatoria que generó el juego tiene que pasar `validate`.
#[test]
fn todas_las_reliquias_naturales_son_legales() {
    let Some((data, relics)) = load() else { return };
    let mut random = 0;
    let mut failures = Vec::new();
    for r in &relics {
        let kind = relic_kind(&data, r.relic.item_id);
        if r.kind_label == "UniqueRelic" {
            assert!(
                !kind.is_random(),
                "{} figura como única pero es aleatoria",
                r.name
            );
            continue;
        }
        if !kind.is_random() {
            continue;
        }
        random += 1;
        if let Err(v) = validate(&data, &r.relic) {
            failures.push(format!("{}: {v} ({:?})", r.name, r.relic.slots));
        }
    }
    eprintln!("reliquias aleatorias validadas: {random}");
    assert!(random > 0);
    assert!(
        failures.is_empty(),
        "reliquias naturales rechazadas:\n{}",
        failures.join("\n")
    );
}

/// Simula transferencias entre reliquias reales: cada plan aceptado tiene que dar una reliquia legal,
/// y tiene que haber al menos una transferencia posible.
/// Listas de SpEffects leídas de la memoria del juego (objeto + 0x5C) el 2026-11-04.
#[test]
fn lista_de_sp_effects_coincide_con_el_juego() {
    let Some((data, _)) = load() else { return };
    /// (item, efectos, maldiciones, SpEffects leídos del juego)
    type Case = (u32, [u32; 4], [u32; 4], &'static [u32]);
    let e = u32::MAX;
    let cases: [Case; 5] = [
        // Deep con 2 maldiciones: primero los efectos, después las maldiciones.
        (
            2011321,
            [6005600, 6040100, e, e],
            [6851400, 6820100, e, e],
            &[7005601, 7040101, 6851400, 6820100],
        ),
        // Efecto con 3 SpEffects al final.
        (
            2030,
            [7000090, 7010200, 7050100, e],
            [e; 4],
            &[7000090, 7010200, 7050100, 7050200, 7050300],
        ),
        // Efecto con 3 SpEffects en el medio.
        (
            1460,
            [7031200, 7050100, 7126000, e],
            [e; 4],
            &[7031200, 7050100, 7050200, 7050300, 7126000, 7126001],
        ),
        // Repetidos: no se eliminan.
        (
            1520,
            [7126000, 7126001, 7126002, e],
            [e; 4],
            &[7126000, 7126001, 7126000, 7126001, 7126000, 7126001],
        ),
        // Deep sin maldición con efecto de 2 SpEffects.
        (
            2013311,
            [6647000, 6622400, e, e],
            [e; 4],
            &[7647000, 7622400, 7622401],
        ),
    ];
    for (item, effects, curses, expected) in cases {
        let relic = Relic::from_raw(item, effects, curses);
        assert_eq!(relic.sp_effects(&data).unwrap(), expected, "item {item}");
    }
}

/// Los tipos normales del grupo 250 (`100xxxx`) son aleatorios y se forjan: salen de los drops de
/// las expediciones (`ItemLotParam_map` y `_enemy`) y el juego los deja vender (probado el
/// 2026-11-11). Antes se los tomaba por "únicos del Bazaar" y quedaban fuera de la forja.
#[test]
fn reliquias_del_grupo_250_son_aleatorias() {
    let Some((data, _)) = load() else { return };
    let group_250: Vec<_> = data
        .relic_types
        .values()
        .filter(|t| !t.is_deep && t.sort_group == 250 && (1_000_000..2_000_000).contains(&t.id))
        .collect();
    assert!(group_250.len() > 300, "tipos 100xxxx: {}", group_250.len());
    for t in group_250 {
        assert!(relic_kind(&data, t.id).is_random(), "{} ({}) no es aleatoria", t.name, t.id);
    }
}

/// Deep con otra mezcla de espacios con y sin maldición: el tipo hermano se ve igual y es del mismo
/// bloque de IDs. Los casos de la prueba en el juego del 2026-11-06.
#[test]
fn deep_cambia_al_tipo_hermano() {
    let Some((data, _)) = load() else { return };
    // Physical Attack Up +3 y Fire Damage Negation +1 (potenciados, con maldición) y Poison
    // Resistance +1 (sin maldición).
    let (atk, fire, poison) = (6_001_400, 6_002_700, 6_003_000);
    let (c1, c2) = (6_820_000, 6_820_100);
    for (both_cursed, mixed, all_plain) in [
        (2_001_001, 2_000_001, 2_003_001), // bloque 200 (tabla 2100000)
        (2_011_001, 2_010_001, 2_013_001), // bloque 201 (tabla 2200000)
    ] {
        let siblings = deep_siblings(&data, both_cursed);
        assert!(siblings.contains(&mixed) && siblings.contains(&all_plain));
        let t = &data.relic_types;
        for s in &siblings {
            assert_eq!(
                (&t[s].name, t[s].color, t[s].icon_id),
                (&t[&both_cursed].name, t[&both_cursed].color, t[&both_cursed].icon_id)
            );
        }

        // Maldito + maldito, y el segundo pasa a ser uno sin maldición: tipo mezclado.
        let mut relic = Relic::new(
            both_cursed,
            [Slot::new(atk, Some(c1)), Slot::new(fire, Some(c2)), Slot::EMPTY],
        );
        normalize(&data, &mut relic);
        assert_eq!(validate(&data, &relic), Ok(()));
        let mut result = relic.clone();
        let i = result.slots.iter().position(|s| s.effect == Some(fire)).unwrap();
        result.slots[i] = Slot::new(poison, None);
        normalize(&data, &mut result);
        assert!(validate(&data, &result).is_err());
        let found = siblings.iter().find(|&&id| {
            let mut r = result.clone();
            r.item_id = id;
            normalize(&data, &mut r);
            validate(&data, &r).is_ok()
        });
        assert_eq!(found, Some(&mixed), "desde {both_cursed}");
    }
}

#[test]
fn transferencias_entre_reliquias_reales() {
    let Some((data, relics)) = load() else { return };
    let grands: Vec<&Relic> = relics
        .iter()
        .map(|r| &r.relic)
        .filter(|r| {
            relic_kind(&data, r.item_id)
                == RelicKind::Random {
                    deep: false,
                    size: 3,
                }
        })
        .collect();
    assert!(grands.len() >= 5, "hacen falta al menos 5 grandes normales");

    let rules = Rules::default();
    let (mut ok, mut rejected) = (0, 0);
    for (di, donor) in grands.iter().enumerate() {
        for (ti, target) in grands.iter().enumerate() {
            if di == ti {
                continue;
            }
            let sacs: Vec<&Relic> = grands
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != di && *i != ti)
                .map(|(_, r)| *r)
                .take(3)
                .collect();
            for donor_slot in 0..3 {
                for target_slot in 0..3 {
                    let req = TransferRequest {
                        donor,
                        donor_slot,
                        target,
                        target_slot,
                        target_forged: &[],
                        target_forge_count: 0,
                        sacrifices: &sacs,
                        available_murk: 1_000_000,
                    };
                    match plan_transfer(&data, &rules, &req) {
                        Ok(plan) => {
                            ok += 1;
                            assert_eq!(validate(&data, &plan.new_target), Ok(()));
                            assert!(plan
                                .new_target
                                .effects()
                                .any(|e| Some(e) == donor.slots[donor_slot].effect));
                        }
                        Err(TransferError::ResultInvalid(_)) => rejected += 1,
                        Err(e) => panic!("error inesperado: {e}"),
                    }
                }
            }
        }
    }
    eprintln!("transferencias simuladas: {ok} válidas, {rejected} rechazadas por reglas del juego");
    assert!(ok > 0);
}
