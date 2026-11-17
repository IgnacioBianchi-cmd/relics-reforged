//! Configuración del jugador: `relics_reforged.ini`, en la carpeta `RelicsReforged` junto a la DLL.
//!
//! Si no existe, se crea con los valores por defecto (así una actualización del mod, que pisa la
//! carpeta, no se lleva la configuración: el zip no trae el `.ini`). Lo que falte o esté mal en
//! el archivo queda con su valor por defecto y se avisa en el log.
//!
//! Las claves van en inglés y los comentarios en los dos idiomas, como en los perfiles de me3:
//! el archivo lo edita el jugador.

use std::path::Path;
use std::sync::OnceLock;

use hudhook::imgui::Key;
use relic_rules::lang::Lang;
use relic_rules::Rules;

use crate::pad;

pub const FILE_NAME: &str = "relics_reforged.ini";

/// Máximo de sacrificios por forja.
pub const MAX_SACRIFICES: usize = 9;

/// Teclas que pueden abrir la forja: ninguna se usa dentro de la forja ni al escribir en la
/// búsqueda. (nombre en el `.ini`, tecla de ImGui, glifo del atlas del juego).
const OPEN_KEYS: [(&str, Key, &str); 18] = [
    ("F1", Key::F1, "KG_Key_F1"),
    ("F2", Key::F2, "KG_Key_F2"),
    ("F3", Key::F3, "KG_Key_F3"),
    ("F4", Key::F4, "KG_Key_F4"),
    ("F5", Key::F5, "KG_Key_F5"),
    ("F6", Key::F6, "KG_Key_F6"),
    ("F7", Key::F7, "KG_Key_F7"),
    ("F8", Key::F8, "KG_Key_F8"),
    ("F9", Key::F9, "KG_Key_F9"),
    ("F10", Key::F10, "KG_Key_F10"),
    ("F11", Key::F11, "KG_Key_F11"),
    ("F12", Key::F12, "KG_Key_F12"),
    ("Insert", Key::Insert, "KG_Key_Insert"),
    ("Delete", Key::Delete, "KG_Key_Delete"),
    ("Home", Key::Home, "KG_Key_Home"),
    ("End", Key::End, "KG_Key_End"),
    ("PageUp", Key::PageUp, "KG_Key_PageUp"),
    ("PageDown", Key::PageDown, "KG_Key_PageDown"),
];

/// Botones del mando que pueden abrir la forja: Relic Rites y la forja no usan los gatillos.
const OPEN_BUTTONS: [(&str, u16); 4] = [
    ("R2", pad::RIGHT_TRIGGER),
    ("RT", pad::RIGHT_TRIGGER),
    ("L2", pad::LEFT_TRIGGER),
    ("LT", pad::LEFT_TRIGGER),
];

#[derive(Debug, Clone)]
pub struct Config {
    pub rules: Rules,
    /// Tecla que abre la forja: nombre, tecla de ImGui y glifo.
    pub open_key: (&'static str, Key, &'static str),
    /// Botón del mando que abre la forja (bits de `pad`).
    pub open_button: u16,
    /// Idioma del menú; `None` = el del juego.
    pub language: Option<Lang>,
    /// Copias del save que se guardan (una por sesión, antes de la primera forja). 0 = ninguna.
    pub save_backups: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            rules: Rules::default(),
            open_key: OPEN_KEYS[6],
            open_button: pad::OPEN_BUTTON,
            language: None,
            save_backups: 10,
        }
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// La configuración cargada (la por defecto si todavía no se cargó).
pub fn get() -> &'static Config {
    CONFIG.get_or_init(Config::default)
}

/// Lee el `.ini` de `dir` (lo crea si no existe) y deja la configuración para
/// [`get`]. Devuelve las líneas para el log.
pub fn load(dir: &Path) -> Vec<String> {
    let path = dir.join(FILE_NAME);
    let mut log = Vec::new();
    let config = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let (config, warnings) = parse(&text);
            log.push(format!("Configuración leída de {}", path.display()));
            log.extend(warnings.into_iter().map(|w| format!("  AVISO: {w}")));
            config
        }
        Err(_) => {
            // Con CRLF, para el Bloc de notas viejo.
            match std::fs::write(&path, DEFAULT_FILE.replace('\n', "\r\n")) {
                Ok(()) => log.push(format!(
                    "Configuración: no había {FILE_NAME}, creado con los valores por defecto"
                )),
                Err(e) => log.push(format!(
                    "Configuración: no pude crear {} ({e}); uso los valores por defecto",
                    path.display()
                )),
            }
            Config::default()
        }
    };
    log.push(format!("  {}", summary(&config)));
    let _ = CONFIG.set(config);
    log
}

fn summary(c: &Config) -> String {
    let r = &c.rules;
    format!(
        "tecla {}, botón {:#06X}, sacrificios {}, puntos {:?}, murk {:?} (Deep ×{}), \
         límite de forjas {:?}, idioma {:?}, respaldos {}",
        c.open_key.0,
        c.open_button,
        r.sacrifice_count,
        r.min_points,
        r.murk_cost,
        multiplier_text(r.deep_murk_percent),
        r.forge_limit,
        c.language,
        c.save_backups
    )
}

/// Interpreta el `.ini`. Lo que no se entiende queda con el valor por defecto y suma un aviso.
fn parse(text: &str) -> (Config, Vec<String>) {
    let mut c = Config::default();
    let mut warnings = Vec::new();
    let mut section = String::new();
    // Algunos editores guardan UTF-8 con BOM: sin sacarlo, la primera línea no se entiende.
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    for (n, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            warnings.push(format!("línea {}: no entiendo \"{line}\"", n + 1));
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().trim_matches('"');
        let result = apply(&mut c, &section, &key, value);
        if let Err(e) = result {
            warnings.push(format!("línea {} ({key} = {value}): {e}", n + 1));
        }
    }
    check_points(&mut c, &mut warnings);
    (c, warnings)
}

/// Saca un comentario (`;` o `#`) del final de la línea.
fn strip_comment(line: &str) -> &str {
    line.find([';', '#']).map_or(line, |i| &line[..i])
}

fn apply(c: &mut Config, section: &str, key: &str, value: &str) -> Result<(), String> {
    match (section, key) {
        ("controls", "open_key") => {
            c.open_key = *OPEN_KEYS
                .iter()
                .find(|(name, ..)| name.eq_ignore_ascii_case(value))
                .ok_or_else(|| {
                    let names: Vec<&str> = OPEN_KEYS.iter().map(|k| k.0).collect();
                    format!("tecla no permitida (valen: {})", names.join(", "))
                })?;
        }
        ("controls", "open_button") => {
            c.open_button = OPEN_BUTTONS
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(value))
                .map(|&(_, b)| b)
                .ok_or("botón no permitido (valen: R2, L2)")?;
        }
        ("rules", "sacrifices") => {
            let n: usize = number(value)?;
            if n > MAX_SACRIFICES {
                return Err(format!("van de 0 a {MAX_SACRIFICES}"));
            }
            c.rules.sacrifice_count = n;
        }
        ("rules", "min_points") => c.rules.min_points = three(value)?,
        ("rules", "murk_cost") => c.rules.murk_cost = three(value)?,
        ("rules", "deep_murk_multiplier") => c.rules.deep_murk_percent = multiplier(value)?,
        ("rules", "forge_limit") => c.rules.forge_limit = three(value)?,
        ("general", "language") => {
            c.language = match value.to_ascii_lowercase().as_str() {
                "auto" | "" => None,
                "en" | "english" => Some(Lang::En),
                "es" | "spanish" | "español" => Some(Lang::Es),
                _ => return Err("valen: auto, en, es".into()),
            };
        }
        ("general", "save_backups") => c.save_backups = number(value)?,
        _ => return Err("clave desconocida".into()),
    }
    Ok(())
}

/// Un número; acepta separadores de miles (`2,000`, `2.000`, `2_000`) solo si hay un único valor.
fn number<T: std::str::FromStr>(value: &str) -> Result<T, String> {
    let clean: String = value
        .chars()
        .filter(|c| !matches!(c, '_' | '.' | ',' | ' '))
        .collect();
    clean
        .parse()
        .map_err(|_| format!("\"{value}\" no es un número válido"))
}

/// Un multiplicador con hasta 2 decimales (`1.5`, `1,75`, `2`), en centésimos: `1.5` → 150.
fn multiplier(value: &str) -> Result<u32, String> {
    let bad = || format!("\"{value}\" no es un multiplicador válido (por ejemplo 1.5)");
    let (whole, frac) = value.split_once(['.', ',']).unwrap_or((value, ""));
    if frac.len() > 2 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad());
    }
    let whole: u32 = whole.trim().parse().map_err(|_| bad())?;
    let frac: u32 = format!("{frac:0<2}").parse().map_err(|_| bad())?;
    whole
        .checked_mul(100)
        .and_then(|w| w.checked_add(frac))
        .ok_or_else(bad)
}

/// Un multiplicador en centésimos como se escribe en el `.ini`: 150 → `1.5`, 200 → `2`.
fn multiplier_text(percent: u32) -> String {
    let text = format!("{}.{:02}", percent / 100, percent % 100);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Tres números separados por comas (receptora de 1, 2 y 3 efectos).
fn three<T: std::str::FromStr + Copy>(value: &str) -> Result<[T; 3], String> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    let [a, b, c] = parts[..] else {
        return Err("hacen falta 3 valores separados por comas".into());
    };
    Ok([number(a)?, number(b)?, number(c)?])
}

/// Si los puntos mínimos no se pueden juntar con los sacrificios elegidos (cada uno vale hasta
/// 3), la forja sería imposible: se bajan al máximo posible.
fn check_points(c: &mut Config, warnings: &mut Vec<String>) {
    let max = c.rules.sacrifice_count as u32 * 3;
    for (i, p) in c.rules.min_points.iter_mut().enumerate() {
        if *p > max {
            warnings.push(format!(
                "min_points de {} efecto(s) = {p}, pero {} sacrificio(s) suman como mucho {max}: \
                 uso {max}",
                i + 1,
                c.rules.sacrifice_count
            ));
            *p = max;
        }
    }
}

/// El archivo que se crea si no existe. Tiene los mismos valores que [`Config::default`] (lo
/// verifica un test).
pub const DEFAULT_FILE: &str = "\
; Relics Reforged - settings / configuración
;
; Changes apply the next time you start the game. A missing or invalid value uses its default
; (relics_reforged.log says which). Delete this file to get all the defaults back.
;
; Los cambios se aplican la próxima vez que abras el juego. Un valor que falte o esté mal usa
; el valor por defecto (relics_reforged.log dice cuál). Borrá este archivo para volver a todos
; los valores por defecto.

[Controls]
; Key that opens the forge in Relic Rites / Tecla que abre la forja en Relic Rites:
;   F1-F12, Insert, Delete, Home, End, PageUp, PageDown
open_key = F7
; Controller button / Botón del mando: R2 (RT) or/o L2 (LT)
open_button = R2

[Rules]
; Sacrifice relics per forge (0-9). Each one is worth 1, 2 or 3 points (1, 2 or 3 effects).
; Reliquias de sacrificio por forja (0-9). Cada una vale 1, 2 o 3 puntos (1, 2 o 3 efectos).
sacrifices = 3
; The values below go by the target's size: 1 effect, 2 effects, 3 effects.
; Los valores de abajo van según el tamaño de la receptora: 1 efecto, 2 efectos, 3 efectos.
;
; Minimum sacrifice points / Puntos mínimos de sacrificio
min_points = 3, 5, 7
; Murk cost / Costo en murk
murk_cost = 2000, 4000, 7000
; Murk cost multiplier for Deep relics, decimals allowed (1.75)
; Multiplicador del murk para reliquias Deep, se aceptan decimales (1.75)
deep_murk_multiplier = 1.5
; Times a relic can be forged (0 = no limit) / Veces que se puede forjar una reliquia (0 = sin límite)
forge_limit = 1, 1, 2

[General]
; Menu language / Idioma del menú: auto (the game's / el del juego), en, es
language = auto
; Save backups to keep in %APPDATA%\\RelicsReforged\\backups (one per session, before the first
; forge; 0 = no backups).
; Copias del save que se guardan en %APPDATA%\\RelicsReforged\\backups (una por sesión, antes de
; la primera forja; 0 = sin copias).
save_backups = 10
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_matches_defaults() {
        let (c, warnings) = parse(DEFAULT_FILE);
        assert!(warnings.is_empty(), "{warnings:?}");
        let d = Config::default();
        assert_eq!(c.rules, d.rules);
        assert_eq!(c.open_key.0, d.open_key.0);
        assert_eq!(c.open_button, d.open_button);
        assert_eq!(c.language, d.language);
        assert_eq!(c.save_backups, d.save_backups);
    }

    #[test]
    fn custom_values() {
        let (c, warnings) = parse(
            "[controls]\nopen_key = f9\nopen_button = LT\n[Rules]\nsacrifices = 1 ; uno\n\
             min_points = 1,2,3\nmurk_cost = 0, 1.000, 2_500\nforge_limit = 0, 0, 0\n\
             [General]\nlanguage = es\nsave_backups = 0\n",
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(c.open_key.0, "F9");
        assert_eq!(c.open_button, pad::LEFT_TRIGGER);
        assert_eq!(c.rules.sacrifice_count, 1);
        assert_eq!(c.rules.min_points, [1, 2, 3]);
        assert_eq!(c.rules.murk_cost, [0, 1_000, 2_500]);
        assert_eq!(c.rules.forge_limit, [0, 0, 0]);
        assert_eq!(c.language, Some(Lang::Es));
        assert_eq!(c.save_backups, 0);
    }

    #[test]
    fn deep_multiplier_accepts_decimals() {
        for (text, percent) in [("1.5", 150), ("1,75", 175), ("2", 200), ("0", 0), ("1.05", 105)] {
            let (c, warnings) = parse(&format!("[Rules]\ndeep_murk_multiplier = {text}\n"));
            assert!(warnings.is_empty(), "{text}: {warnings:?}");
            assert_eq!(c.rules.deep_murk_percent, percent, "{text}");
            assert_eq!(multiplier_text(percent), text.replace(',', "."));
        }
        for bad in ["1.555", "x", "-1", "1.5x", ""] {
            let (c, warnings) = parse(&format!("[Rules]\ndeep_murk_multiplier = {bad}\n"));
            assert_eq!(warnings.len(), 1, "{bad}: {warnings:?}");
            assert_eq!(c.rules.deep_murk_percent, Rules::default().deep_murk_percent);
        }
    }

    #[test]
    fn bad_values_keep_defaults() {
        let (c, warnings) = parse(
            "[Controls]\nopen_key = Q\n[Rules]\nsacrifices = 12\nmurk_cost = 1, 2\nfoo = 1\n",
        );
        assert_eq!(warnings.len(), 4, "{warnings:?}");
        let d = Config::default();
        assert_eq!(c.open_key.0, d.open_key.0);
        assert_eq!(c.rules, d.rules);
    }

    #[test]
    fn utf8_bom_is_ignored() {
        let (c, warnings) = parse("\u{FEFF}[General]\nsave_backups = 2\n");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(c.save_backups, 2);
    }

    #[test]
    fn impossible_points_are_lowered() {
        let (c, warnings) = parse("[Rules]\nsacrifices = 1\n");
        // 3 se puede juntar con un sacrificio grande; 5 y 7, no.
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert_eq!(c.rules.min_points, [3, 3, 3]);
    }
}
