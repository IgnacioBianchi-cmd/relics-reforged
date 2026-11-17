//! Aviso en pantalla cuando el mod no puede funcionar con esta versión del juego.
//!
//! Las direcciones del mod son de `nightreign.exe` 1.3.3.0: con otra versión (por ejemplo, después
//! de una actualización del juego) el mod no instala nada y queda inactivo. Para que el jugador se
//! entere sin abrir el log, se muestra este aviso durante el primer minuto de juego. No toca nada
//! del juego: solo dibuja (el gancho de DirectX 12 de hudhook no depende de la versión).

use std::path::Path;
use std::time::{Duration, Instant};

use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::imgui::{self, Condition, Ui};
use hudhook::windows::Win32::Foundation::HINSTANCE;
use hudhook::{Hudhook, ImguiRenderLoop};
use relic_rules::lang::Lang;

use crate::config;

/// El AppID de Nightreign en Steam: su manifiesto dice en qué idioma está instalado el juego.
const STEAM_APP_ID: u32 = 2622380;

extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
}

/// Cuánto se muestra, desde el primer frame.
const SHOW_FOR: Duration = Duration::from_secs(60);
/// Los últimos segundos se desvanece.
const FADE: Duration = Duration::from_secs(5);

struct Notice {
    lines: Vec<String>,
    first: Option<Instant>,
}

impl ImguiRenderLoop for Notice {
    fn render(&mut self, ui: &mut Ui) {
        let since = self.first.get_or_insert_with(Instant::now).elapsed();
        if since > SHOW_FOR {
            return;
        }
        let left = SHOW_FOR - since;
        let alpha = if left < FADE {
            left.as_secs_f32() / FADE.as_secs_f32()
        } else {
            1.0
        };
        let w = ui.io().display_size[0];
        let v = crate::ui::View::of(ui);
        let s = v.s;
        let _bg = ui.push_style_color(imgui::StyleColor::WindowBg, [0.04, 0.05, 0.08, 0.88 * alpha]);
        let _border = ui.push_style_color(imgui::StyleColor::Border, [0.85, 0.70, 0.40, 0.8 * alpha]);
        let _pad = ui.push_style_var(imgui::StyleVar::WindowPadding([18.0 * s, 14.0 * s]));
        ui.window("##relics_reforged_notice")
            .position([w * 0.5, v.y(40.0)], Condition::Always)
            .position_pivot([0.5, 0.0])
            .no_decoration()
            .no_inputs()
            .focus_on_appearing(false)
            .always_auto_resize(true)
            .build(|| {
                ui.set_window_font_scale(1.6 * s);
                for (i, line) in self.lines.iter().enumerate() {
                    let color = if i == 0 {
                        [0.95, 0.80, 0.45, alpha]
                    } else {
                        [0.88, 0.87, 0.82, alpha]
                    };
                    ui.text_colored(color, line);
                }
            });
    }
}

/// Idioma del aviso. Con otra versión del juego no se leen sus textos (que es como se elige el
/// idioma del menú), así que sale de: la configuración del jugador; si no, el idioma con que
/// Steam instaló el juego (el que usa el juego); si no, el de Windows.
fn notice_lang() -> (Lang, &'static str) {
    if let Some(lang) = config::get().language {
        return (lang, "configuración");
    }
    let exe = crate::win::module_path(crate::win::main_module_base()).unwrap_or_default();
    // <biblioteca>\steamapps\common\ELDEN RING NIGHTREIGN\Game\nightreign.exe
    let manifest = exe
        .ancestors()
        .nth(4)
        .map(|steamapps| steamapps.join(format!("appmanifest_{STEAM_APP_ID}.acf")));
    if let Some(lang) = manifest.as_deref().and_then(steam_lang) {
        return (lang, "Steam");
    }
    // SAFETY: no recibe nada; devuelve un LANGID.
    let windows = unsafe { GetUserDefaultUILanguage() };
    // El idioma primario son los 10 bits bajos; 0x0A es español.
    let lang = if windows & 0x3FF == 0x0A { Lang::Es } else { Lang::En };
    (lang, "Windows")
}

fn steam_lang(manifest: &Path) -> Option<Lang> {
    manifest_lang(&std::fs::read_to_string(manifest).ok()?)
}

/// El idioma de un `appmanifest_*.acf`: el de `MountedConfig` (con el que está instalado) o, si
/// no está, el de `UserConfig`. Steam llama `spanish` al de España y `latam` al de Latinoamérica.
fn manifest_lang(text: &str) -> Option<Lang> {
    let value = |section: &str| {
        let rest = &text[text.find(section)?..];
        let rest = &rest[rest.find("\"language\"")? + "\"language\"".len()..];
        let start = rest.find('"')? + 1;
        let end = start + rest[start..].find('"')?;
        Some(rest[start..end].to_ascii_lowercase())
    };
    let name = value("\"MountedConfig\"").or_else(|| value("\"UserConfig\""))?;
    Some(if matches!(name.as_str(), "spanish" | "latam") {
        Lang::Es
    } else {
        Lang::En
    })
}

/// Muestra el aviso de versión no compatible, en el idioma del juego (ver [`notice_lang`]).
pub fn unsupported_version(module: usize) {
    let (lang, source) = notice_lang();
    crate::log_event(&format!("Aviso de versión en {lang:?} (idioma según {source})"));
    let lines: Vec<String> = match lang {
        Lang::En => [
            "Relics Reforged is disabled",
            "This version of Nightreign is not supported yet (the mod needs game version 1.3.3).",
            "The game runs normally without the mod. Check Nexus Mods for an update.",
        ],
        Lang::Es => [
            "Relics Reforged está desactivado",
            "Esta versión de Nightreign todavía no es compatible (el mod necesita la versión 1.3.3).",
            "El juego funciona normal sin el mod. Buscá una actualización en Nexus Mods.",
        ],
    }
    .map(String::from)
    .to_vec();
    std::thread::spawn(move || {
        let result = Hudhook::builder()
            .with::<ImguiDx12Hooks>(Notice { lines, first: None })
            .with_hmodule(HINSTANCE(module as *mut _))
            .build()
            .apply();
        if let Err(e) = result {
            crate::log_event(&format!("ERROR: no pude mostrar el aviso de versión: {e:?}"));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "\"AppState\"
{
	\"appid\"		\"2622380\"
	\"UserConfig\"
	{
		\"language\"		\"english\"
	}
	\"MountedConfig\"
	{
		\"language\"		\"latam\"
	}
}
";

    #[test]
    fn idioma_del_manifiesto_de_steam() {
        // Manda el idioma con que está instalado.
        assert_eq!(manifest_lang(MANIFEST), Some(Lang::Es));
        let user_only = "\"UserConfig\"
{
\"language\"	\"spanish\"
}";
        assert_eq!(manifest_lang(user_only), Some(Lang::Es));
        assert_eq!(manifest_lang(&MANIFEST.replace("latam", "french")), Some(Lang::En));
        assert_eq!(manifest_lang("\"AppState\" {}"), None);
    }
}
