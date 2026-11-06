//! Aviso en pantalla cuando el mod no puede funcionar con esta versión del juego.
//!
//! Las direcciones del mod son de `nightreign.exe` 1.3.3.0: con otra versión (por ejemplo, después
//! de una actualización del juego) el mod no instala nada y queda inactivo. Para que el jugador se
//! entere sin abrir el log, se muestra este aviso durante el primer minuto de juego. No toca nada
//! del juego: solo dibuja (el gancho de DirectX 12 de hudhook no depende de la versión).

use std::time::{Duration, Instant};

use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::imgui::{self, Condition, Ui};
use hudhook::windows::Win32::Foundation::HINSTANCE;
use hudhook::{Hudhook, ImguiRenderLoop};

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
        let [w, h] = ui.io().display_size;
        let s = h / 1080.0;
        let _bg = ui.push_style_color(imgui::StyleColor::WindowBg, [0.04, 0.05, 0.08, 0.88 * alpha]);
        let _border = ui.push_style_color(imgui::StyleColor::Border, [0.85, 0.70, 0.40, 0.8 * alpha]);
        let _pad = ui.push_style_var(imgui::StyleVar::WindowPadding([18.0 * s, 14.0 * s]));
        ui.window("##relics_reforged_notice")
            .position([w * 0.5, 40.0 * s], Condition::Always)
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

/// Muestra el aviso de versión no compatible (en inglés y en español: sin los textos del juego
/// no se sabe su idioma).
pub fn unsupported_version(module: usize) {
    let lines = vec![
        "Relics Reforged is disabled".to_string(),
        "This version of Nightreign is not supported yet (the mod needs game version 1.3.3).".to_string(),
        "The game runs normally without the mod. Check Nexus Mods for an update.".to_string(),
        String::new(),
        "Relics Reforged está desactivado".to_string(),
        "Esta versión de Nightreign todavía no es compatible (el mod necesita la versión 1.3.3).".to_string(),
        "El juego funciona normal sin el mod. Buscá una actualización en Nexus Mods.".to_string(),
    ];
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
