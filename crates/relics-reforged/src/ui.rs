//! Menú dentro del juego (Dear ImGui sobre DirectX 12, con hudhook).
//!
//! Se abre y se cierra con F7. Mientras está abierto, el teclado y el mouse no le llegan al juego.
//! El menú corre en el hilo de render: todo lo que lee o escribe datos del juego se hace en hilos
//! aparte (la forja, a su vez, en el hilo principal), para no trabar el frame.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::imgui::{self, Condition, Key, Ui};
use hudhook::windows::Win32::Foundation::HINSTANCE;
use hudhook::{Hudhook, ImguiRenderLoop, MessageFilter};
use relic_rules::lang::tr;
use relic_rules::{relic_kind, EffectId, GameData, Relic, RelicKind};

use crate::forge::{self, Order};
use crate::game::{is_favorite, Game};

pub const TOGGLE_KEY_NAME: &str = "F7";
const TOGGLE_KEY: Key = Key::F7;

const RED: [f32; 4] = [1.0, 0.45, 0.4, 1.0];
const GREEN: [f32; 4] = [0.5, 0.9, 0.5, 1.0];
const GOLD: [f32; 4] = [0.95, 0.8, 0.45, 1.0];
/// Texto de los efectos en las listas: más apagado que los nombres.
const EFFECT: [f32; 4] = [0.72, 0.71, 0.66, 1.0];

/// Arranca el overlay en un hilo propio.
pub fn start(module: usize, game: Game, data: Arc<GameData>, dll_dir: PathBuf) {
    std::thread::spawn(move || {
        let menu = Menu::new(game, data, dll_dir);
        let result = Hudhook::builder()
            .with::<ImguiDx12Hooks>(menu)
            .with_hmodule(HINSTANCE(module as *mut _))
            .build()
            .apply();
        match result {
            Ok(()) => crate::log_event("Menú instalado (DirectX 12)"),
            Err(e) => crate::log_event(&format!("ERROR: no pude instalar el menú: {e:?}")),
        }
    });
}

/// Una reliquia tal como la muestra el menú.
#[derive(Clone)]
struct RelicView {
    handle: u32,
    name: String,
    /// `None` si no es aleatoria (no sirve para forjar).
    size: Option<usize>,
    deep: bool,
    /// `relicColor`: 0 Burning, 1 Drizzly, 2 Luminous, 3 Tranquil.
    color: u8,
    effects: Vec<String>,
    /// Por slot: si ese efecto ya fue forjado por el mod.
    forged: Vec<bool>,
    /// Por qué no se puede donar ni sacrificar (favorita, equipada, preset, recipiente).
    locked: Option<String>,
}

impl RelicView {
    fn label(&self) -> String {
        format!("{}  ·  {}", self.name, self.effects.join("  |  "))
    }

    fn forged_count(&self) -> usize {
        self.forged.iter().filter(|f| **f).count()
    }
}

/// Aviso chico, abajo a la derecha, mientras Relic Rites está abierto.
fn draw_hint(ui: &Ui) {
    let [w, h] = ui.io().display_size;
    ui.window("##relics_reforged_hint")
        .position([w - 24.0, h - 24.0], Condition::Always)
        .position_pivot([1.0, 1.0])
        .bg_alpha(0.6)
        .no_decoration()
        .no_inputs()
        .always_auto_resize(true)
        .build(|| {
            ui.text_colored(
                GOLD,
                format!(
                    "[{TOGGLE_KEY_NAME}] {}",
                    tr("Forjar reliquias", "Forge relics")
                ),
            );
        });
}

/// Parte un texto en líneas que entren en `width` (corta por palabras).
fn wrap_text(ui: &Ui, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if !current.is_empty() && ui.calc_text_size(&candidate)[0] > width {
            lines.push(std::mem::replace(&mut current, word.to_string()));
        } else {
            current = candidate;
        }
    }
    lines.push(current);
    lines
}

/// Texto apagado que baja de línea si no entra.
fn hint(ui: &Ui, text: &str) {
    let color = ui.style_color(imgui::StyleColor::TextDisabled);
    colored_wrapped(ui, color, text);
}

/// Texto en color que baja de línea si no entra.
fn colored_wrapped(ui: &Ui, color: [f32; 4], text: &str) {
    let _c = ui.push_style_color(imgui::StyleColor::Text, color);
    ui.text_wrapped(text);
}

#[derive(Default)]
struct Shared {
    relics: Vec<RelicView>,
    murk: u32,
    loading: bool,
    load_error: Option<String>,
    /// Vista previa de la selección actual: (clave de la selección, resultado de `forge?`).
    preview: Option<(SelectionKey, Result<String, String>)>,
    preview_busy: bool,
    /// Resultado de la última forja.
    last: Option<Result<String, String>>,
    forging: bool,
}

/// `FontId` es un puntero a la fuente dentro del atlas de ImGui; solo se usa en el hilo de render.
#[derive(Clone, Copy)]
struct TitleFont(imgui::FontId);
// SAFETY: el atlas vive tanto como el contexto de ImGui y el id solo se usa al dibujar.
unsafe impl Send for TitleFont {}
unsafe impl Sync for TitleFont {}

type SelectionKey = (u32, usize, u32, usize, [u32; 3]);

struct Menu {
    visible: AtomicBool,
    game: Game,
    data: Arc<GameData>,
    dll_dir: PathBuf,
    shared: Arc<Mutex<Shared>>,
    filter: String,
    target: Option<u32>,
    target_slot: usize,
    donor: Option<u32>,
    donor_slot: usize,
    sacrifices: Vec<u32>,
    confirm: bool,
    /// Fuente de los títulos de sección (Palatino en negrita), si se pudo cargar.
    title_font: Option<TitleFont>,
}

impl Menu {
    fn new(game: Game, data: Arc<GameData>, dll_dir: PathBuf) -> Menu {
        Menu {
            visible: AtomicBool::new(false),
            game,
            data,
            dll_dir,
            shared: Arc::new(Mutex::new(Shared::default())),
            filter: String::new(),
            target: None,
            target_slot: 0,
            donor: None,
            donor_slot: 0,
            sacrifices: Vec::new(),
            confirm: false,
            title_font: None,
        }
    }

    fn shared(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Vuelve a leer las reliquias del juego en un hilo aparte.
    fn refresh(&self) {
        {
            let mut s = self.shared();
            if s.loading {
                return;
            }
            s.loading = true;
        }
        let (game, data, dir, shared) = (
            self.game,
            self.data.clone(),
            self.dll_dir.clone(),
            self.shared.clone(),
        );
        std::thread::spawn(move || {
            let result = load(&game, &data, &dir);
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            match result {
                Ok((relics, murk)) => {
                    s.relics = relics;
                    s.murk = murk;
                    s.load_error = None;
                }
                Err(e) => s.load_error = Some(e),
            }
            s.loading = false;
            s.preview = None;
        });
    }

    fn reset_selection(&mut self) {
        self.target = None;
        self.donor = None;
        self.sacrifices.clear();
        self.target_slot = 0;
        self.donor_slot = 0;
        self.confirm = false;
    }

    fn selection_key(&self) -> Option<SelectionKey> {
        let (target, donor) = (self.target?, self.donor?);
        let s: [u32; 3] = self.sacrifices.clone().try_into().ok()?;
        Some((donor, self.donor_slot, target, self.target_slot, s))
    }

    fn order(key: SelectionKey, dry_run: bool) -> Order {
        let (donor, donor_slot, target, target_slot, sacrifices) = key;
        Order {
            donor,
            donor_slot: donor_slot + 1,
            target,
            target_slot: target_slot + 1,
            sacrifices,
            dry_run,
        }
    }

    /// Pide la vista previa (`forge?`) si la selección está completa y cambió.
    fn update_preview(&self) {
        let Some(key) = self.selection_key() else {
            return;
        };
        {
            let mut s = self.shared();
            if s.preview_busy || s.forging || s.preview.as_ref().is_some_and(|(k, _)| *k == key) {
                return;
            }
            s.preview_busy = true;
        }
        let (game, data, dir, shared) = (
            self.game,
            self.data.clone(),
            self.dll_dir.clone(),
            self.shared.clone(),
        );
        std::thread::spawn(move || {
            let result = forge::forge(game, &data, &dir, Menu::order(key, true));
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            s.preview = Some((key, result));
            s.preview_busy = false;
        });
    }

    fn start_forge(&mut self, key: SelectionKey) {
        {
            let mut s = self.shared();
            if s.forging {
                return;
            }
            s.forging = true;
            s.last = None;
        }
        let (game, data, dir, shared) = (
            self.game,
            self.data.clone(),
            self.dll_dir.clone(),
            self.shared.clone(),
        );
        std::thread::spawn(move || {
            let result = forge::forge(game, &data, &dir, Menu::order(key, false));
            match &result {
                Ok(msg) => crate::log_event(&format!("[menú] {msg}")),
                Err(e) => crate::log_event(&format!("[menú] ERROR: {e}")),
            }
            let ok = result.is_ok();
            {
                let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                s.last = Some(result);
                s.forging = false;
                s.preview = None;
            }
            if ok {
                // Las reliquias cambiaron: hay que volver a leerlas.
                if let Ok((relics, murk)) = load(&game, &data, &dir) {
                    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                    s.relics = relics;
                    s.murk = murk;
                }
            }
        });
        self.reset_selection();
    }

    fn draw(&mut self, ui: &Ui) {
        let [w, h] = ui.io().display_size;
        let size = [
            (w * 0.72).clamp(860.0_f32.min(w - 40.0), w - 40.0),
            (h * 0.78).clamp(560.0_f32.min(h - 40.0), h - 40.0),
        ];
        let mut open = true;
        ui.window("Relics Reforged")
            .size(size, Condition::Appearing)
            .position([w * 0.5, h * 0.5], Condition::Appearing)
            .position_pivot([0.5, 0.5])
            .collapsible(false)
            .opened(&mut open)
            .build(|| self.draw_contents(ui));
        if !open {
            self.visible.store(false, Ordering::Relaxed);
        }
    }

    /// Título de sección, en dorado y con la fuente grande si se pudo cargar.
    fn section(&self, ui: &Ui, title: &str) {
        let _f = self.title_font.map(|f| ui.push_font(f.0));
        ui.text_colored(GOLD, title);
        drop(_f);
        ui.separator();
    }

    fn draw_contents(&mut self, ui: &Ui) {
        let (relics, murk, loading, load_error, preview, preview_busy, last, forging) = {
            let s = self.shared();
            (
                s.relics.clone(),
                s.murk,
                s.loading,
                s.load_error.clone(),
                s.preview.clone(),
                s.preview_busy,
                s.last.clone(),
                s.forging,
            )
        };
        let by_handle: HashMap<u32, &RelicView> = relics.iter().map(|r| (r.handle, r)).collect();

        // Encabezado: murk, cantidad, botones y filtro.
        ui.text_colored(GOLD, format!("Murk  {murk}"));
        ui.same_line_with_spacing(0.0, 28.0);
        ui.text_disabled(format!("{}: {}", tr("Reliquias", "Relics"), relics.len()));
        ui.same_line_with_spacing(0.0, 28.0);
        if ui.button(tr("Refrescar", "Refresh")) {
            self.refresh();
        }
        ui.same_line();
        if ui.button(tr("Empezar de nuevo", "Start over")) {
            self.reset_selection();
        }
        ui.same_line_with_spacing(0.0, 28.0);
        ui.set_next_item_width(-1.0);
        ui.input_text("##filtro", &mut self.filter)
            .hint(tr(
                "Filtrar por nombre o efecto",
                "Filter by name or effect",
            ))
            .build();
        if loading {
            hint(ui, tr("Leyendo reliquias...", "Reading relics..."));
        }
        if let Some(e) = &load_error {
            colored_wrapped(
                ui,
                RED,
                &format!(
                    "{}: {e}",
                    tr("No pude leer las reliquias", "Couldn't read the relics")
                ),
            );
        }
        match &last {
            Some(Ok(msg)) => colored_wrapped(ui, GREEN, msg),
            Some(Err(e)) => colored_wrapped(ui, RED, e),
            None => {}
        }
        if forging {
            ui.text_colored(GOLD, tr("Forjando...", "Forging..."));
        }
        ui.spacing();

        // Dos columnas, como Relic Rites: la selección a la izquierda y la lista a la derecha.
        let avail = ui.content_region_avail();
        let left_w = (avail[0] * 0.42).max(340.0);
        ui.child_window("izquierda")
            .size([left_w, 0.0])
            .border(true)
            .build(|| self.draw_left(ui, &by_handle, preview.as_ref(), preview_busy, forging));
        ui.same_line();
        ui.child_window("derecha")
            .size([0.0, 0.0])
            .border(true)
            .build(|| self.draw_right(ui, &relics));
    }

    fn draw_left(
        &mut self,
        ui: &Ui,
        by_handle: &HashMap<u32, &RelicView>,
        preview: Option<&(SelectionKey, Result<String, String>)>,
        preview_busy: bool,
        forging: bool,
    ) {
        // Receptora.
        self.section(ui, tr("Receptora", "Target"));
        match self.target.and_then(|h| by_handle.get(&h).copied()) {
            None => hint(
                ui,
                tr(
                    "Elegila en la lista de la derecha.",
                    "Choose it from the list on the right.",
                ),
            ),
            Some(t) => {
                relic_title(ui, t);
                ui.same_line();
                if ui.small_button(format!("{}##receptora", tr("cambiar", "change"))) {
                    self.reset_selection();
                    return;
                }
                hint(ui, tr("Efecto que se reemplaza:", "Effect to replace:"));
                self.slot_picker(ui, "r", t, true);
            }
        }
        ui.spacing();

        // Donante.
        self.section(ui, tr("Donante", "Donor"));
        match (
            self.target,
            self.donor.and_then(|h| by_handle.get(&h).copied()),
        ) {
            (None, _) => hint(
                ui,
                tr("Primero elegí la receptora.", "Choose the target first."),
            ),
            (Some(_), None) => hint(
                ui,
                tr(
                    "Elegila en la lista de la derecha. Se destruye.",
                    "Choose it from the list on the right. It is destroyed.",
                ),
            ),
            (Some(_), Some(d)) => {
                relic_title(ui, d);
                ui.same_line();
                if ui.small_button(format!("{}##donante", tr("cambiar", "change"))) {
                    self.donor = None;
                    self.sacrifices.clear();
                    self.confirm = false;
                    return;
                }
                hint(
                    ui,
                    tr(
                        "Efecto que se transfiere (con su maldición):",
                        "Effect to transfer (with its curse):",
                    ),
                );
                self.slot_picker(ui, "d", d, false);
            }
        }
        ui.spacing();

        // Sacrificios.
        let points: u32 = self
            .sacrifices
            .iter()
            .filter_map(|h| by_handle.get(h))
            .map(|r| r.size.unwrap_or(0) as u32)
            .sum();
        self.section(
            ui,
            &format!(
                "{} ({}/3, {points} {})",
                tr("Sacrificios", "Sacrifices"),
                self.sacrifices.len(),
                tr("puntos", "points")
            ),
        );
        if self.donor.is_none() {
            hint(ui, tr("Después de la donante.", "After the donor."));
        } else if self.sacrifices.is_empty() {
            hint(
                ui,
                tr(
                    "Elegí 3 en la lista de la derecha (chica 1, mediana 2, grande 3).",
                    "Choose 3 from the list on the right (delicate 1, polished 2, grand 3).",
                ),
            );
        }
        let mut remove = None;
        for &h in &self.sacrifices {
            if let Some(r) = by_handle.get(&h) {
                relic_title(ui, r);
                ui.same_line();
                if ui.small_button(format!("x##sacrificio{h:X}")) {
                    remove = Some(h);
                }
            }
        }
        if let Some(h) = remove {
            self.sacrifices.retain(|&x| x != h);
            self.confirm = false;
        }
        ui.spacing();

        // Resultado y forja.
        self.section(ui, tr("Resultado", "Result"));
        self.update_preview();
        let Some(key) = self.selection_key() else {
            hint(
                ui,
                tr(
                    "Completá la selección para ver el resultado.",
                    "Complete the selection to see the result.",
                ),
            );
            return;
        };
        match preview.filter(|(k, _)| *k == key) {
            None => {
                if preview_busy {
                    hint(ui, tr("Calculando...", "Calculating..."));
                }
            }
            Some((_, Err(e))) => colored_wrapped(ui, RED, e),
            Some((_, Ok(msg))) => {
                // El efecto transferido se resalta en dorado.
                let moved = self
                    .donor
                    .and_then(|h| by_handle.get(&h))
                    .and_then(|d| d.effects.get(self.donor_slot));
                for line in msg.lines() {
                    match line.strip_prefix(EFFECT_LINE) {
                        Some(e) if Some(&e.to_string()) == moved => {
                            colored_wrapped(ui, GOLD, &format!("   {e}"))
                        }
                        Some(e) => ui.text_wrapped(format!("   {e}")),
                        None => ui.text_wrapped(line),
                    }
                }
                if forging {
                    return;
                }
                ui.spacing();
                if !self.confirm {
                    if ui.button_with_size(tr("Forjar", "Forge"), [140.0, 0.0]) {
                        self.confirm = true;
                    }
                } else {
                    colored_wrapped(
                        ui,
                        RED,
                        tr(
                            "La donante y los 3 sacrificios se destruyen. No se puede deshacer.",
                            "The donor and the 3 sacrifices are destroyed. This can't be undone.",
                        ),
                    );
                    if ui.button(tr("Confirmar forja", "Confirm forge")) {
                        self.start_forge(key);
                    }
                    ui.same_line();
                    if ui.button(tr("Cancelar", "Cancel")) {
                        self.confirm = false;
                    }
                }
            }
        }
    }

    /// La lista del paso actual: receptoras, donantes o sacrificios.
    fn draw_right(&mut self, ui: &Ui, relics: &[RelicView]) {
        let target = self
            .target
            .and_then(|h| relics.iter().find(|r| r.handle == h));
        let chosen: HashSet<u32> = self.sacrifices.iter().copied().collect();
        if target.is_none() {
            self.section(
                ui,
                tr(
                    "Elegí la reliquia que querés mejorar",
                    "Choose the relic you want to improve",
                ),
            );
            let list: Vec<&RelicView> = relics
                .iter()
                .filter(|r| r.size.is_some() && self.matches_filter(r))
                .collect();
            if let Some(h) = self.relic_list(ui, "receptoras", &list, &HashSet::new(), true) {
                self.target = Some(h);
                self.target_slot = 0;
            }
            return;
        }
        let t = target.expect("receptora elegida");
        if self.donor.is_none() {
            self.section(
                ui,
                tr(
                    "Elegí la donante (se destruye)",
                    "Choose the donor (it is destroyed)",
                ),
            );
            let list: Vec<&RelicView> = relics
                .iter()
                .filter(|r| {
                    r.size.is_some()
                        && r.deep == t.deep
                        && r.handle != t.handle
                        && r.locked.is_none()
                        && self.matches_filter(r)
                })
                .collect();
            if let Some(h) = self.relic_list(ui, "donantes", &list, &HashSet::new(), false) {
                self.donor = Some(h);
                self.donor_slot = 0;
            }
            return;
        }
        let donor = self.donor.expect("donante elegida");
        self.section(
            ui,
            tr(
                "Elegí 3 sacrificios (se destruyen)",
                "Choose 3 sacrifices (they are destroyed)",
            ),
        );
        let list: Vec<&RelicView> = relics
            .iter()
            .filter(|r| {
                r.size.is_some()
                    && r.deep == t.deep
                    && r.handle != t.handle
                    && r.handle != donor
                    && r.locked.is_none()
                    && (chosen.contains(&r.handle) || self.matches_filter(r))
            })
            .collect();
        if let Some(h) = self.relic_list(ui, "sacrificios", &list, &chosen, false) {
            if let Some(pos) = self.sacrifices.iter().position(|&x| x == h) {
                self.sacrifices.remove(pos);
            } else if self.sacrifices.len() < 3 {
                self.sacrifices.push(h);
            }
            self.confirm = false;
        }
    }

    fn matches_filter(&self, r: &RelicView) -> bool {
        let f = self.filter.trim().to_lowercase();
        f.is_empty() || r.label().to_lowercase().contains(&f)
    }

    /// Lista seleccionable de reliquias, con la marca de color de cada una. Devuelve el handle
    /// clickeado.
    fn relic_list(
        &self,
        ui: &Ui,
        id: &str,
        list: &[&RelicView],
        selected: &HashSet<u32>,
        show_locks: bool,
    ) -> Option<u32> {
        let mut clicked = None;
        ui.child_window(id).size([0.0, 0.0]).build(|| {
            if list.is_empty() {
                hint(
                    ui,
                    tr("No hay reliquias que sirvan acá.", "No usable relics here."),
                );
            }
            let line = ui.text_line_height();
            let text = ui.style_color(imgui::StyleColor::Text);
            for r in list {
                // Una fila por reliquia: el nombre arriba y cada efecto debajo (los largos bajan
                // de línea), como el detalle de Relic Rites. El texto se dibuja encima de un
                // selectable vacío del alto de la fila.
                let [x, y] = ui.cursor_screen_pos();
                let width = ui.content_region_avail()[0];
                let effect_x = x + 32.0;
                let wrap_w = (x + width - effect_x - 6.0).max(80.0);
                let mut effect_lines: Vec<(f32, [f32; 4], String)> = Vec::new();
                for (i, e) in r.effects.iter().enumerate() {
                    let color = if r.forged.get(i) == Some(&true) {
                        GOLD
                    } else {
                        EFFECT
                    };
                    for (j, l) in wrap_text(ui, e, wrap_w).into_iter().enumerate() {
                        // Las continuaciones van un poco más adentro.
                        let indent = if j == 0 { 0.0 } else { 14.0 };
                        effect_lines.push((indent, color, l));
                    }
                }
                let lines = 1 + effect_lines.len().max(1);
                let row_h = line * lines as f32 + 6.0;
                if ui
                    .selectable_config(format!("##{id}{:X}", r.handle))
                    .selected(selected.contains(&r.handle))
                    .size([0.0, row_h])
                    .build()
                {
                    clicked = Some(r.handle);
                }
                let y = y + 3.0;
                color_mark(ui, [x + 4.0, y + line * 0.5], r.color);
                let draw = ui.get_window_draw_list();
                let name_x = x + 20.0;
                draw.add_text([name_x, y], text, &r.name);
                let mut tag_x = name_x + ui.calc_text_size(&r.name)[0] + 14.0;
                if r.forged_count() > 0 {
                    let tag = format!("[{} {}]", tr("forjada", "forged"), r.forged_count());
                    draw.add_text([tag_x, y], GOLD, &tag);
                    tag_x += ui.calc_text_size(&tag)[0] + 10.0;
                }
                if show_locks {
                    if let Some(why) = &r.locked {
                        let tag = format!("[{}: {why}]", tr("en uso", "in use"));
                        draw.add_text([tag_x, y], RED, &tag);
                    }
                }
                for (i, (indent, color, l)) in effect_lines.iter().enumerate() {
                    draw.add_text([effect_x + indent, y + line * (i + 1) as f32], *color, l);
                }
            }
        });
        clicked
    }

    /// Botones de radio con los efectos de una reliquia (y su maldición, si tiene).
    fn slot_picker(&mut self, ui: &Ui, id: &str, r: &RelicView, target: bool) {
        let slots = r.size.unwrap_or(0);
        for i in 0..slots {
            let mut text = r
                .effects
                .get(i)
                .cloned()
                .unwrap_or_else(|| tr("(vacío)", "(empty)").into());
            if r.forged.get(i) == Some(&true) {
                text.push_str(tr("  [forjado]", "  [forged]"));
            }
            let value = if target {
                &mut self.target_slot
            } else {
                &mut self.donor_slot
            };
            // El texto va aparte del botón para que baje de línea si no entra (las maldiciones
            // suelen ser largas); clickear el texto también elige el efecto.
            let mut changed = ui.radio_button(format!("##{id}{i}"), value, i);
            ui.same_line();
            ui.text_wrapped(&text);
            if ui.is_item_clicked() && *value != i {
                *value = i;
                changed = true;
            }
            if changed {
                self.confirm = false;
            }
        }
    }
}

/// Color de la escena de la reliquia (`relicColor`): 0 Burning, 1 Drizzly, 2 Luminous, 3 Tranquil.
fn relic_color(color: u8) -> [f32; 4] {
    match color {
        0 => [0.86, 0.27, 0.22, 1.0],
        1 => [0.30, 0.52, 0.92, 1.0],
        2 => [0.93, 0.78, 0.30, 1.0],
        3 => [0.36, 0.76, 0.48, 1.0],
        _ => [0.6, 0.6, 0.6, 1.0],
    }
}

/// Rombo de color centrado verticalmente en `at` (borde izquierdo).
fn color_mark(ui: &Ui, at: [f32; 2], color: u8) {
    let c = relic_color(color);
    let (x, y, r) = (at[0] + 5.0, at[1], 5.0);
    let draw = ui.get_window_draw_list();
    draw.add_triangle([x - r, y], [x, y - r], [x + r, y], c)
        .filled(true)
        .build();
    draw.add_triangle([x - r, y], [x, y + r], [x + r, y], c)
        .filled(true)
        .build();
}

/// Marca de color y nombre de una reliquia, con sus forjas si tiene.
fn relic_title(ui: &Ui, r: &RelicView) {
    let p = ui.cursor_screen_pos();
    let h = ui.text_line_height();
    color_mark(ui, [p[0], p[1] + h * 0.5], r.color);
    ui.dummy([14.0, h]);
    ui.same_line();
    ui.text(&r.name);
    if r.forged_count() > 0 {
        ui.same_line();
        ui.text_colored(
            GOLD,
            format!("[{} {}]", tr("forjada", "forged"), r.forged_count()),
        );
    }
}

impl ImguiRenderLoop for Menu {
    fn initialize<'a>(
        &'a mut self,
        ctx: &mut imgui::Context,
        _render_context: &'a mut dyn hudhook::RenderContext,
    ) {
        apply_theme(ctx.style_mut());
        // Palatino, parecida a la tipografía de los menús del juego; Georgia si no está.
        let fonts_dir = std::env::var("WINDIR")
            .map_or(PathBuf::from(r"C:\Windows"), PathBuf::from)
            .join("Fonts");
        let load = |names: &[&str]| -> Option<&'static [u8]> {
            let bytes = names
                .iter()
                .find_map(|n| std::fs::read(fonts_dir.join(n)).ok())?;
            Some(Box::leak(bytes.into_boxed_slice()))
        };
        let source = |data: &'static [u8], size_pixels: f32| imgui::FontSource::TtfData {
            data,
            size_pixels,
            config: Some(imgui::FontConfig {
                oversample_h: 2,
                ..Default::default()
            }),
        };
        match load(&["pala.ttf", "georgia.ttf"]) {
            Some(data) => {
                ctx.fonts().add_font(&[source(data, 19.0)]);
            }
            None => {
                crate::log_event("Menú: no encontré Palatino ni Georgia, uso la fuente de ImGui")
            }
        }
        if let Some(data) = load(&["palab.ttf", "georgiab.ttf"]) {
            self.title_font = Some(TitleFont(ctx.fonts().add_font(&[source(data, 21.0)])));
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        // La forja vive dentro de Relic Rites: fuera de ese menú no se muestra nada.
        if !self.game.relic_rites_open() {
            if self.visible.swap(false, Ordering::Relaxed) {
                self.reset_selection();
            }
            return;
        }
        if ui.is_key_pressed(TOGGLE_KEY) {
            let now = !self.visible.load(Ordering::Relaxed);
            self.visible.store(now, Ordering::Relaxed);
            if now {
                self.refresh();
            }
        }
        if self.visible.load(Ordering::Relaxed) {
            self.draw(ui);
        } else {
            draw_hint(ui);
        }
    }

    fn before_render<'a>(
        &'a mut self,
        ctx: &mut imgui::Context,
        _render_context: &'a mut dyn hudhook::RenderContext,
    ) {
        // Con el menú abierto, ImGui dibuja su propio cursor (el juego suele esconder el de Windows).
        ctx.io_mut().mouse_draw_cursor = self.visible.load(Ordering::Relaxed);
    }

    fn message_filter(&self, _io: &imgui::Io) -> MessageFilter {
        if self.visible.load(Ordering::Relaxed) {
            MessageFilter::InputAll
        } else {
            MessageFilter::empty()
        }
    }
}

/// Lee reliquias, bloqueos y murk.
fn load(
    game: &Game,
    data: &GameData,
    dir: &std::path::Path,
) -> Result<(Vec<RelicView>, u32), String> {
    let murk = game
        .murk()
        .ok_or(tr("la partida no está cargada", "the game is not loaded"))?;
    let live = game.relics();
    if live.is_empty() {
        return Err(tr(
            "no encuentro reliquias (¿la partida está cargada?)",
            "no relics found (is the game loaded?)",
        )
        .into());
    }
    let mut locks: HashMap<u32, Vec<String>> = HashMap::new();
    match game.relic_references() {
        Ok(refs) => {
            for (h, place) in refs {
                // "vessel 0xBB9" -> "vessel": el ID interno no le dice nada al jugador.
                let place = match place.split_once(" 0x") {
                    Some((kind, _)) => kind.to_string(),
                    None => place,
                };
                let places = locks.entry(h).or_default();
                if !places.contains(&place) {
                    places.push(place);
                }
            }
        }
        // Si no se puede verificar, todas cuentan como en uso.
        Err(e) => {
            for r in &live {
                locks.entry(r.handle).or_default().push(e.clone());
            }
        }
    }
    let registry = forge::ForgedRegistry::load(dir);
    let mut order: HashMap<u32, u32> = HashMap::new();
    for (_, entry) in game.inventory_entries() {
        order.insert(entry[0], entry[3]);
        if is_favorite(&entry) {
            locks
                .entry(entry[0])
                .or_default()
                .push(tr("favorita", "favorite").into());
        }
    }

    let mut views: Vec<RelicView> = live
        .into_iter()
        .map(|r| {
            let relic = Relic::from_raw(r.item_id, r.effects, r.curses);
            let (size, deep) = match relic_kind(data, r.item_id) {
                RelicKind::Random { deep, size } => (Some(size), deep),
                _ => (None, false),
            };
            let effects = relic
                .slots
                .iter()
                .filter_map(|s| Some(effect_text(data, s.effect?, s.curse)))
                .collect();
            let forged_ids = order.get(&r.handle).map_or(&[][..], |o| registry.get(*o));
            let forged = relic
                .slots
                .iter()
                .filter_map(|s| s.effect)
                .map(|e| forged_ids.contains(&e))
                .collect();
            let kind = data.relic_types.get(&r.item_id);
            RelicView {
                handle: r.handle,
                forged,
                name: kind.map_or("?".into(), |t| t.name.clone()),
                color: kind.map_or(u8::MAX, |t| t.color),
                size,
                deep,
                effects,
                locked: locks.get(&r.handle).map(|p| p.join(", ")),
            }
        })
        .collect();
    views.sort_by(|a, b| a.name.cmp(&b.name).then(a.handle.cmp(&b.handle)));
    Ok((views, murk))
}

/// Prefijo de cada línea de efecto en la vista previa de `forge?` (la UI las reconoce por esto).
pub(crate) const EFFECT_LINE: &str = "- ";

/// Nombre de un efecto con su maldición, si tiene: "Fire Attack Power Up +3 (curse: ...)".
pub(crate) fn effect_text(data: &GameData, effect: EffectId, curse: Option<EffectId>) -> String {
    let name = |e: EffectId| {
        data.effects
            .get(&e)
            .map_or(e.to_string(), |i| clean_name(&i.name))
    };
    match curse {
        Some(c) => format!(
            "{} ({}: {})",
            name(effect),
            tr("maldición", "curse"),
            name(c)
        ),
        None => name(effect),
    }
}

/// "[Relic] Strength +3" -> "Strength +3"; "[Relic - Wylder] X" -> "(Wylder) X".
pub(crate) fn clean_name(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix("[Relic] ") {
        return rest.to_string();
    }
    if let Some(rest) = raw.strip_prefix("[Relic - ") {
        if let Some((who, text)) = rest.split_once("] ") {
            return format!("({who}) {text}");
        }
    }
    raw.to_string()
}

/// Colores oscuros con bordes dorados, parecidos a los menús del juego.
fn apply_theme(style: &mut imgui::Style) {
    use imgui::StyleColor as C;
    let gold_line = [0.45, 0.40, 0.28, 0.55];
    for (c, v) in [
        (C::WindowBg, [0.06, 0.07, 0.10, 0.94]),
        (C::ChildBg, [0.08, 0.09, 0.13, 0.70]),
        (C::PopupBg, [0.07, 0.08, 0.11, 0.96]),
        (C::Border, gold_line),
        (C::Separator, gold_line),
        (C::TitleBg, [0.07, 0.08, 0.12, 1.0]),
        (C::TitleBgActive, [0.12, 0.13, 0.20, 1.0]),
        (C::TitleBgCollapsed, [0.07, 0.08, 0.12, 1.0]),
        (C::Text, [0.86, 0.85, 0.80, 1.0]),
        (C::TextDisabled, [0.55, 0.54, 0.50, 1.0]),
        (C::Header, [0.20, 0.28, 0.45, 0.75]),
        (C::HeaderHovered, [0.28, 0.36, 0.56, 0.80]),
        (C::HeaderActive, [0.32, 0.42, 0.65, 0.90]),
        (C::Button, [0.16, 0.18, 0.26, 1.0]),
        (C::ButtonHovered, [0.26, 0.30, 0.42, 1.0]),
        (C::ButtonActive, [0.34, 0.38, 0.52, 1.0]),
        (C::FrameBg, [0.10, 0.11, 0.16, 1.0]),
        (C::FrameBgHovered, [0.16, 0.18, 0.26, 1.0]),
        (C::FrameBgActive, [0.20, 0.23, 0.32, 1.0]),
        (C::CheckMark, GOLD),
        (C::ScrollbarBg, [0.05, 0.06, 0.08, 0.6]),
        (C::ScrollbarGrab, [0.30, 0.28, 0.22, 0.8]),
        (C::ScrollbarGrabHovered, [0.42, 0.38, 0.28, 0.9]),
        (C::ScrollbarGrabActive, [0.55, 0.48, 0.32, 1.0]),
    ] {
        style[c] = v;
    }
    style.window_rounding = 0.0;
    style.child_rounding = 0.0;
    style.frame_rounding = 2.0;
    style.window_padding = [14.0, 12.0];
    style.item_spacing = [8.0, 6.0];
    style.window_title_align = [0.5, 0.5];
}
