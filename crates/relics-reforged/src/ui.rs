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
use relic_rules::{relic_kind, EffectId, GameData, Relic, RelicKind};

use crate::forge::{self, Order};
use crate::game::{is_favorite, Game};

pub const TOGGLE_KEY_NAME: &str = "F7";
const TOGGLE_KEY: Key = Key::F7;

const RED: [f32; 4] = [1.0, 0.45, 0.4, 1.0];
const GREEN: [f32; 4] = [0.5, 0.9, 0.5, 1.0];
const GOLD: [f32; 4] = [0.95, 0.8, 0.45, 1.0];

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
        let mut open = true;
        ui.window("Relics Reforged")
            .size([760.0, 820.0], Condition::FirstUseEver)
            .position([60.0, 60.0], Condition::FirstUseEver)
            .opened(&mut open)
            .build(|| self.draw_contents(ui));
        if !open {
            self.visible.store(false, Ordering::Relaxed);
        }
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

        ui.text(format!("Murk: {murk}    Reliquias: {}", relics.len()));
        ui.same_line();
        if ui.button("Refrescar") {
            self.refresh();
        }
        ui.same_line();
        if ui.button("Empezar de nuevo") {
            self.reset_selection();
        }
        if loading {
            ui.text_disabled("Leyendo reliquias...");
        }
        if let Some(e) = &load_error {
            ui.text_colored(RED, format!("No pude leer las reliquias: {e}"));
        }
        match &last {
            Some(Ok(msg)) => colored_wrapped(ui, GREEN, msg),
            Some(Err(e)) => colored_wrapped(ui, RED, e),
            None => {}
        }
        if forging {
            ui.text_colored(GOLD, "Forjando...");
        }
        ui.input_text("Filtro", &mut self.filter).build();
        ui.separator();

        // 1. Receptora.
        let target = self.target.and_then(|h| by_handle.get(&h).copied());
        match target {
            None => {
                ui.text_colored(GOLD, "1. Elegí la reliquia que querés mejorar (receptora)");
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
            Some(t) => {
                ui.text(format!("Receptora: {}", t.name));
                ui.same_line();
                if ui.small_button("cambiar##receptora") {
                    self.reset_selection();
                    return;
                }
                ui.text_colored(GOLD, "2. Efecto que se reemplaza:");
                self.slot_picker(ui, "r", t, true);
            }
        }
        let t = target.expect("receptora elegida");
        ui.separator();

        // 3. Donante.
        let donor = self.donor.and_then(|h| by_handle.get(&h).copied());
        match donor {
            None => {
                ui.text_colored(GOLD, "3. Elegí la donante (se destruye)");
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
            Some(d) => {
                ui.text(format!("Donante: {}", d.name));
                ui.same_line();
                if ui.small_button("cambiar##donante") {
                    self.donor = None;
                    self.sacrifices.clear();
                    self.confirm = false;
                    return;
                }
                ui.text_colored(GOLD, "4. Efecto que se transfiere (con su maldición):");
                self.slot_picker(ui, "d", d, false);
            }
        }
        let d = donor.expect("donante elegida");
        ui.separator();

        // 5. Sacrificios.
        let points: u32 = self
            .sacrifices
            .iter()
            .filter_map(|h| by_handle.get(h))
            .map(|r| r.size.unwrap_or(0) as u32)
            .sum();
        ui.text_colored(
            GOLD,
            format!(
                "5. Elegí 3 sacrificios ({}/3, {points} puntos; chica 1, mediana 2, grande 3)",
                self.sacrifices.len()
            ),
        );
        let chosen: HashSet<u32> = self.sacrifices.iter().copied().collect();
        let list: Vec<&RelicView> = relics
            .iter()
            .filter(|r| {
                r.size.is_some()
                    && r.deep == t.deep
                    && r.handle != t.handle
                    && r.handle != d.handle
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
        ui.separator();

        // 6. Vista previa y forja.
        self.update_preview();
        let Some(key) = self.selection_key() else {
            ui.text_disabled("Completá la selección para ver el resultado.");
            return;
        };
        match preview.as_ref().filter(|(k, _)| *k == key) {
            None => {
                if preview_busy {
                    ui.text_disabled("Calculando...");
                }
            }
            Some((_, Err(e))) => colored_wrapped(ui, RED, e),
            Some((_, Ok(msg))) => {
                ui.text_wrapped(msg);
                if forging {
                    return;
                }
                if !self.confirm {
                    if ui.button("Forjar") {
                        self.confirm = true;
                    }
                } else {
                    ui.text_colored(
                        RED,
                        "La donante y los 3 sacrificios se destruyen. No se puede deshacer.",
                    );
                    if ui.button("Confirmar forja") {
                        self.start_forge(key);
                    }
                    ui.same_line();
                    if ui.button("Cancelar") {
                        self.confirm = false;
                    }
                }
            }
        }
    }

    fn matches_filter(&self, r: &RelicView) -> bool {
        let f = self.filter.trim().to_lowercase();
        f.is_empty() || r.label().to_lowercase().contains(&f)
    }

    /// Lista seleccionable de reliquias. Devuelve el handle clickeado.
    fn relic_list(
        &self,
        ui: &Ui,
        id: &str,
        list: &[&RelicView],
        selected: &HashSet<u32>,
        show_locks: bool,
    ) -> Option<u32> {
        let mut clicked = None;
        ui.child_window(id)
            .size([0.0, 260.0])
            .border(true)
            .horizontal_scrollbar(true)
            .build(|| {
                if list.is_empty() {
                    ui.text_disabled("No hay reliquias que sirvan acá.");
                }
                for r in list {
                    let mut label = r.label();
                    if r.forged_count() > 0 {
                        label.push_str(&format!("  [forjada {}]", r.forged_count()));
                    }
                    if show_locks && r.locked.is_some() {
                        label.push_str("  [en uso]");
                    }
                    if ui
                        .selectable_config(format!("{label}##{id}{:X}", r.handle))
                        .selected(selected.contains(&r.handle))
                        .build()
                    {
                        clicked = Some(r.handle);
                    }
                    if ui.is_item_hovered() {
                        ui.tooltip(|| {
                            ui.text(&r.name);
                            for (i, e) in r.effects.iter().enumerate() {
                                let mark = if r.forged.get(i) == Some(&true) {
                                    "  [forjado]"
                                } else {
                                    ""
                                };
                                ui.text(format!("  {}. {e}{mark}", i + 1));
                            }
                            if let Some(why) = &r.locked {
                                ui.text_colored(RED, format!("En uso: {why}"));
                            }
                        });
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
                .unwrap_or_else(|| "(vacío)".into());
            if r.forged.get(i) == Some(&true) {
                text.push_str("  [forjado]");
            }
            let value = if target {
                &mut self.target_slot
            } else {
                &mut self.donor_slot
            };
            if ui.radio_button(format!("{}. {text}##{id}{i}", i + 1), value, i) {
                self.confirm = false;
            }
        }
    }
}

impl ImguiRenderLoop for Menu {
    fn render(&mut self, ui: &mut Ui) {
        if ui.is_key_pressed(TOGGLE_KEY) {
            let now = !self.visible.load(Ordering::Relaxed);
            self.visible.store(now, Ordering::Relaxed);
            if now {
                self.refresh();
            }
        }
        if self.visible.load(Ordering::Relaxed) {
            self.draw(ui);
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
    let murk = game.murk().ok_or("la partida no está cargada")?;
    let live = game.relics();
    if live.is_empty() {
        return Err("no encuentro reliquias (¿la partida está cargada?)".into());
    }
    let mut locks: HashMap<u32, Vec<String>> = HashMap::new();
    match game.relic_references() {
        Ok(refs) => {
            for (h, place) in refs {
                locks.entry(h).or_default().push(place);
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
            locks.entry(entry[0]).or_default().push("favorita".into());
        }
    }
    let effect_name = |e: EffectId| -> String {
        let raw = data
            .effects
            .get(&e)
            .map_or(e.to_string(), |i| i.name.clone());
        clean_name(&raw)
    };
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
                .filter_map(|s| {
                    let e = s.effect?;
                    let mut text = effect_name(e);
                    if let Some(c) = s.curse {
                        text.push_str(&format!(" (maldición: {})", effect_name(c)));
                    }
                    Some(text)
                })
                .collect();
            let forged_ids = order.get(&r.handle).map_or(&[][..], |o| registry.get(*o));
            let forged = relic
                .slots
                .iter()
                .filter_map(|s| s.effect)
                .map(|e| forged_ids.contains(&e))
                .collect();
            RelicView {
                handle: r.handle,
                forged,
                name: data
                    .relic_types
                    .get(&r.item_id)
                    .map_or("?".into(), |t| t.name.clone()),
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
