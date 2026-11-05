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

use nr_assets::IconAtlas;

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
    let pending: Arc<Mutex<Option<IconAtlas>>> = Arc::default();
    load_icons(&data, pending.clone());
    std::thread::spawn(move || {
        let mut menu = Menu::new(game, data, dll_dir);
        menu.pending_icons = pending;
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

/// Glifo de la tecla F7 en el atlas del juego (mismo estilo que F1/F2 en Relic Rites).
const KEY_ICON: &str = "KG_Key_F7";
/// Fondo de panel con bordes difuminados y la línea blanca con brillo de arriba de los paneles
/// de Relic Rites; una línea fina para separar secciones.
const PANEL_BG: &str = "MENU_In_bg_05";
const PANEL_LINE: &str = "MENU_In_Line_02";
const SEP_LINE: &str = "MENU_In_Line_00";
/// Teclas de los controles propios: (glifo, tecla, tecla de ImGui).
const KEY_BACK: (&str, &str, Key) = ("KG_Key_Q", "Q", Key::Q);
const KEY_REFRESH: (&str, &str, Key) = ("KG_Key_R", "R", Key::R);
const KEY_CLEAR: (&str, &str, Key) = ("KG_Key_C", "C", Key::C);
const KEY_FILTER: (&str, &str, Key) = ("KG_Key_4", "4", Key::Alpha4);
const KEY_SORT: (&str, &str, Key) = ("KG_Key_5", "5", Key::Alpha5);
const KEY_RESET: (&str, &str, Key) = ("KG_Key_1", "1", Key::Alpha1);
/// Texto de los controles: un blanco apagado, como el del juego.
const PROMPT_TEXT: [f32; 4] = [0.74, 0.74, 0.72, 1.0];

fn status_icon(id: i32) -> String {
    format!("MENU_StatusIcon_{id}")
}

fn item_icon(id: i32) -> String {
    format!("MENU_ItemIcon_{id}")
}

/// Saca del juego instalado los íconos de efectos y reliquias (y el de la tecla F7), en un hilo
/// aparte: tarda unos segundos y lee ~70 MB. Mientras tanto, el menú se dibuja sin íconos.
fn load_icons(data: &GameData, out: Arc<Mutex<Option<IconAtlas>>>) {
    let mut wanted: Vec<String> = data
        .effects
        .values()
        .filter(|e| e.icon_id >= 0)
        .map(|e| status_icon(e.icon_id))
        .chain(
            data.relic_types
                .values()
                .filter(|t| t.icon_id >= 0)
                .map(|t| item_icon(t.icon_id)),
        )
        .chain(
            [
                KEY_ICON,
                PANEL_BG,
                PANEL_LINE,
                SEP_LINE,
                KEY_BACK.0,
                KEY_REFRESH.0,
                KEY_CLEAR.0,
                KEY_FILTER.0,
                KEY_SORT.0,
                KEY_RESET.0,
            ]
            .map(String::from),
        )
        .collect();
    wanted.sort();
    wanted.dedup();
    std::thread::spawn(move || {
        let Some(game_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(PathBuf::from))
        else {
            return;
        };
        let start = std::time::Instant::now();
        match nr_assets::load_menu_icons(&game_dir, &wanted) {
            Ok(atlas) => {
                crate::log_event(&format!(
                    "Íconos: {} de {} leídos del juego en {:.1} s ({}x{})",
                    atlas.rects.len(),
                    wanted.len(),
                    start.elapsed().as_secs_f32(),
                    atlas.width,
                    atlas.height
                ));
                *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(atlas);
            }
            Err(e) => crate::log_event(&format!(
                "Íconos: no pude leerlos del juego ({e}); el menú sigue sin íconos"
            )),
        }
    });
}

/// El atlas de íconos ya subido a la GPU.
struct Icons {
    texture: imgui::TextureId,
    size: [f32; 2],
    rects: HashMap<String, [u32; 4]>,
}

impl Icons {
    /// Dibuja el ícono `name` en el rectángulo dado. `false` si no existe.
    fn draw(&self, ui: &Ui, name: &str, min: [f32; 2], size: [f32; 2]) -> bool {
        let Some(&[x, y, w, h]) = self.rects.get(name) else {
            return false;
        };
        let [aw, ah] = self.size;
        ui.get_window_draw_list()
            .add_image(self.texture, min, [min[0] + size[0], min[1] + size[1]])
            .uv_min([x as f32 / aw, y as f32 / ah])
            .uv_max([(x + w) as f32 / aw, (y + h) as f32 / ah])
            .build();
        true
    }

    fn has(&self, name: &str) -> bool {
        self.rects.contains_key(name)
    }

    /// Relación ancho/alto del ícono.
    fn aspect(&self, name: &str) -> f32 {
        self.rects
            .get(name)
            .map_or(1.0, |&[_, _, w, h]| w as f32 / h.max(1) as f32)
    }
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
    /// Ícono de la reliquia (`iconId` de `EquipParamAntique`).
    icon: i32,
    effects: Vec<String>,
    /// Ícono de cada efecto (`statusIconId`), en el mismo orden que `effects`. -1 = sin ícono.
    effect_icons: Vec<i32>,
    /// Por slot: si ese efecto ya fue forjado por el mod.
    forged: Vec<bool>,
    /// Por qué no se puede donar ni sacrificar (favorita, equipada, preset, recipiente).
    locked: Option<String>,
    /// Orden en que se encontró (el del inventario del juego); `u32::MAX` si no se sabe.
    order: u32,
    favorite: bool,
}

impl RelicView {
    fn label(&self) -> String {
        format!("{}  ·  {}", self.name, self.effects.join("  |  "))
    }

    fn forged_count(&self) -> usize {
        self.forged.iter().filter(|f| **f).count()
    }
}

/// Un control al estilo del juego: la tecla (su glifo, si está en los íconos) y el texto al
/// lado, en la posición del cursor. Usa la fuente actual. Devuelve `true` si se hizo clic.
fn key_prompt(
    ui: &Ui,
    icons: Option<&Icons>,
    key_icon: &str,
    key: &str,
    label: &str,
    s: f32,
) -> bool {
    let label = format!(":{label}");
    let key_size = ui.calc_text_size(key);
    let label_size = ui.calc_text_size(&label);
    let glyph = icons.filter(|i| i.has(key_icon));
    let box_h = label_size[1] + 6.0 * s;
    let box_w = match glyph {
        Some(i) => box_h * i.aspect(key_icon),
        None => (key_size[0] + 12.0 * s).max(box_h),
    };
    let gap = 3.0 * s;
    let p = ui.cursor_screen_pos();
    let clicked = ui.invisible_button(
        format!("##prompt_{key}"),
        [box_w + gap + label_size[0], box_h],
    );
    let text = if ui.is_item_hovered() {
        GOLD
    } else {
        PROMPT_TEXT
    };
    // Sombra difusa alrededor del texto (varias copias corridas), como los controles del juego.
    let shadow = [0.0, 0.0, 0.0, 0.16];
    let label_at = [p[0] + box_w + gap, p[1] + (box_h - label_size[1]) * 0.5];
    let mut texts = vec![(label_at, label.as_str())];
    if !glyph.is_some_and(|i| i.draw(ui, key_icon, p, [box_w, box_h])) {
        ui.get_window_draw_list()
            .add_rect(p, [p[0] + box_w, p[1] + box_h], [0.80, 0.80, 0.78, 0.9])
            .rounding(3.0 * s)
            .thickness(1.5 * s)
            .build();
        let key_at = [
            p[0] + (box_w - key_size[0]) * 0.5,
            p[1] + (box_h - key_size[1]) * 0.5,
        ];
        texts.push((key_at, key));
    }
    let draw = ui.get_window_draw_list();
    for (at, t) in texts {
        // Un anillo de copias oscuras a 1 y 2,5 px: un halo, no un borde marcado.
        for r in [1.0, 2.5] {
            for (dx, dy) in [
                (1.0, 0.0),
                (0.7, 0.7),
                (0.0, 1.0),
                (-0.7, 0.7),
                (-1.0, 0.0),
                (-0.7, -0.7),
                (0.0, -1.0),
                (0.7, -0.7),
            ] {
                draw.add_text([at[0] + dx * r * s, at[1] + dy * r * s], shadow, t);
            }
        }
        draw.add_text(at, text, t);
    }
    clicked
}

/// Control "F7 :Forge relics" mientras Relic Rites está abierto. Con el juego en inglés y el
/// panel conocido, va en la barra de controles del juego, justo después de "M:All Presets" (la
/// barra es más larga con el cursor en los slots que en la grilla). Si no, va una fila arriba
/// del grupo de abajo a la derecha, donde no tapa nada. Devuelve `true` si se hizo clic.
fn draw_hint(
    ui: &Ui,
    font: Option<TitleFont>,
    icons: Option<&Icons>,
    panel: Option<crate::relic_menu::Panel>,
) -> bool {
    use crate::relic_menu::Panel;
    let [w, h] = ui.io().display_size;
    // Las medidas salen de capturas a 1920x1080; la interfaz del juego escala con el alto y se
    // centra en horizontal.
    let s = h / 1080.0;
    let x = |px: f32| w * 0.5 + (px - 960.0) * s;
    let y = |px: f32| h * 0.5 + (px - 540.0) * s;
    // Fin de "M:All Presets" en la barra de abajo a la izquierda (solo medido con el juego en
    // inglés): la barra es más larga con el cursor en los slots. Eligiendo la reliquia para un
    // slot la barra es solo "1:Swap screen", pero no se distingue de recorrer la grilla: ahí el
    // botón queda en la misma barra, con un hueco a la izquierda.
    let after_presets = match (relic_rules::lang::get(), panel) {
        (relic_rules::lang::Lang::En, Some(Panel::Slots)) => Some(1010.0),
        (relic_rules::lang::Lang::En, Some(Panel::Right)) => Some(527.0),
        _ => None,
    };
    let (pos, pivot) = match after_presets {
        Some(px) => ([x(px), y(1015.0)], [0.0, 0.5]),
        None => ([x(1790.0), y(972.0)], [1.0, 0.5]),
    };
    let mut clicked = false;
    let _pad = ui.push_style_var(imgui::StyleVar::WindowPadding([0.0, 0.0]));
    ui.window("##relics_reforged_hint")
        .position(pos, Condition::Always)
        .position_pivot(pivot)
        .no_decoration()
        .draw_background(false)
        .no_nav()
        .focus_on_appearing(false)
        .always_auto_resize(true)
        .build(|| {
            let _f = font.map(|f| ui.push_font(f.0));
            ui.set_window_font_scale(s);
            clicked = key_prompt(
                ui,
                icons,
                KEY_ICON,
                TOGGLE_KEY_NAME,
                tr("Forjar reliquias", "Forge relics"),
                s,
            );
        });
    clicked
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

/// Filtro de la lista, como el de Relic Rites: se marca lo que se quiere ver. En cada grupo, sin
/// nada marcado no se filtra; con algo marcado, solo pasa lo marcado.
#[derive(Clone, Copy, PartialEq, Default)]
struct Filters {
    favorites: bool,
    /// Por `relicColor`: rojo (Burning), azul (Drizzly), amarillo (Luminous), verde (Tranquil).
    colors: [bool; 4],
    /// Chica, mediana, grande.
    sizes: [bool; 3],
    /// Reliquia normal, reliquia de las profundidades (Deep).
    kinds: [bool; 2],
}

/// ¿Pasa por un grupo del filtro? Sin nada marcado, todo pasa.
fn passes(group: &[bool], index: Option<usize>) -> bool {
    !group.iter().any(|b| *b) || index.and_then(|i| group.get(i)).copied().unwrap_or(false)
}

impl Filters {
    /// Cuántas opciones están marcadas.
    fn active(&self) -> usize {
        let on = |v: &[bool]| v.iter().filter(|b| **b).count();
        self.favorites as usize + on(&self.colors) + on(&self.sizes) + on(&self.kinds)
    }

    fn accepts(&self, r: &RelicView) -> bool {
        (!self.favorites || r.favorite)
            && passes(&self.colors, Some(r.color as usize))
            && passes(&self.sizes, r.size.map(|n| n.wrapping_sub(1)))
            && passes(&self.kinds, Some(r.deep as usize))
    }
}

/// Orden de la lista.
#[derive(Clone, Copy, PartialEq)]
enum SortBy {
    Found,
    Name,
    Color,
    Size,
}

impl SortBy {
    fn label(self) -> &'static str {
        match self {
            SortBy::Found => tr("orden encontrado", "order found"),
            SortBy::Name => tr("nombre", "name"),
            SortBy::Color => tr("color", "color"),
            SortBy::Size => tr("tamaño", "size"),
        }
    }

    fn next(self) -> SortBy {
        match self {
            SortBy::Found => SortBy::Name,
            SortBy::Name => SortBy::Color,
            SortBy::Color => SortBy::Size,
            SortBy::Size => SortBy::Found,
        }
    }

    fn sort(self, list: &mut [&RelicView]) {
        match self {
            SortBy::Found => list.sort_by_key(|r| (r.order, r.handle)),
            SortBy::Name => list.sort_by(|a, b| a.name.cmp(&b.name).then(a.order.cmp(&b.order))),
            SortBy::Color => list.sort_by_key(|r| (r.color, r.order)),
            // Las grandes primero.
            SortBy::Size => list.sort_by_key(|r| (std::cmp::Reverse(r.size), r.order)),
        }
    }
}

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
    prompt_font: Option<TitleFont>,
    /// Atlas leído del juego, esperando que el hilo de render lo suba a la GPU.
    pending_icons: Arc<Mutex<Option<IconAtlas>>>,
    icons: Option<Icons>,
    filters: Filters,
    sort: SortBy,
    /// El panel izquierdo muestra el filtro en vez de la selección (tecla 4).
    show_filters: bool,
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
            prompt_font: None,
            pending_icons: Arc::default(),
            icons: None,
            filters: Filters::default(),
            sort: SortBy::Found,
            show_filters: false,
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

    /// El menú ocupa la pantalla de Relic Rites (salvo su encabezado), como si fuera una sección
    /// más del mismo menú. Abajo lleva su propia barra de controles, como la del juego.
    fn draw(&mut self, ui: &Ui) {
        let [w, h] = ui.io().display_size;
        let s = h / 1080.0;
        // Encabezado del juego ("Relic Rites", arriba a la izquierda): queda a la vista.
        let top = (118.0 * s).round();
        let bg = ui.get_background_draw_list();
        bg.add_rect_filled_multicolor(
            [0.0, top],
            [w, h],
            [0.05, 0.06, 0.10, 0.86],
            [0.05, 0.06, 0.10, 0.86],
            [0.02, 0.03, 0.05, 0.90],
            [0.02, 0.03, 0.05, 0.90],
        );
        bg.add_line([0.0, top], [w, top], [0.80, 0.82, 0.90, 0.18])
            .thickness(1.0)
            .build();
        // Título de la sección al lado de "Relic Rites".
        {
            let _f = self.prompt_font.map(|f| ui.push_font(f.0));
            let title = tr("Forja de reliquias", "Relic Forge");
            let at = [290.0 * s, 67.0 * s - ui.text_line_height() * 0.5];
            bg.add_text(at, [0.55, 0.54, 0.50, 1.0], "/");
            bg.add_text([at[0] + 22.0 * s, at[1]], GOLD, title);
        }
        drop(bg);

        let mut close = false;
        let _pad = ui.push_style_var(imgui::StyleVar::WindowPadding([0.0, 0.0]));
        ui.window("##relics_reforged")
            .position([0.0, top], Condition::Always)
            .size([w, h - top], Condition::Always)
            .no_decoration()
            .draw_background(false)
            .movable(false)
            .bring_to_front_on_focus(false)
            .build(|| {
                // Mismos márgenes que los paneles de Relic Rites.
                let margin = 45.0 * s;
                let bar = 80.0 * s;
                ui.set_cursor_screen_pos([margin, top + 16.0 * s]);
                ui.child_window("contenido")
                    .size([w - margin * 2.0, h - top - 16.0 * s - bar])
                    .build(|| self.draw_contents(ui));
                // Barra de controles, a la altura de la del juego.
                let _f = self.prompt_font.map(|f| ui.push_font(f.0));
                ui.set_window_font_scale(s);
                ui.set_cursor_screen_pos([48.0 * s, h - 80.0 * s]);
                // Un solo "volver": F7 y Esc hacen lo mismo que Q, pero no se muestran. Con el filtro
                // abierto, Q lo cierra y 1 lo limpia (como en Relic Rites).
                let (mut back, mut refresh, mut clear, mut reset) = (false, false, false, false);
                {
                    let icons = self.icons.as_ref();
                    let (g, k, _) = KEY_BACK;
                    if self.show_filters {
                        back |= key_prompt(ui, icons, g, k, tr("Volver", "Back"), s);
                        ui.same_line_with_spacing(0.0, 14.0 * s);
                        let (g, k, _) = KEY_RESET;
                        reset |= key_prompt(ui, icons, g, k, tr("Limpiar", "Reset"), s);
                    } else {
                        back |= key_prompt(
                            ui,
                            icons,
                            g,
                            k,
                            tr("Volver a Relic Rites", "Back to Relic Rites"),
                            s,
                        );
                        ui.same_line_with_spacing(0.0, 14.0 * s);
                        let (g, k, _) = KEY_REFRESH;
                        refresh |= key_prompt(ui, icons, g, k, tr("Refrescar", "Refresh"), s);
                        ui.same_line_with_spacing(0.0, 14.0 * s);
                        let (g, k, _) = KEY_CLEAR;
                        clear |=
                            key_prompt(ui, icons, g, k, tr("Empezar de nuevo", "Start over"), s);
                    }
                }
                ui.set_window_font_scale(1.0);
                if back {
                    if self.show_filters {
                        self.show_filters = false;
                    } else {
                        close = true;
                    }
                }
                if refresh {
                    self.refresh();
                }
                if clear {
                    self.reset_selection();
                }
                if reset {
                    self.filters = Filters::default();
                }
            });
        // Teclas (no mientras se escribe en el filtro: ahí las usa ImGui).
        if !ui.io().want_text_input {
            if ui.is_key_pressed(Key::Escape) || ui.is_key_pressed(KEY_BACK.2) {
                if self.show_filters {
                    self.show_filters = false;
                } else {
                    close = true;
                }
            }
            if self.show_filters && ui.is_key_pressed(KEY_RESET.2) {
                self.filters = Filters::default();
            }
            if !self.show_filters && ui.is_key_pressed(KEY_REFRESH.2) {
                self.refresh();
            }
            if !self.show_filters && ui.is_key_pressed(KEY_CLEAR.2) {
                self.reset_selection();
            }
            if ui.is_key_pressed(KEY_FILTER.2) {
                self.show_filters = !self.show_filters;
            }
            if ui.is_key_pressed(KEY_SORT.2) {
                self.sort = self.sort.next();
            }
        }
        if close {
            self.visible.store(false, Ordering::Relaxed);
        }
    }

    /// Título de sección, en dorado y con la fuente grande si se pudo cargar.
    fn section(&self, ui: &Ui, title: &str) {
        let _f = self.title_font.map(|f| ui.push_font(f.0));
        ui.text_colored(GOLD, title);
        drop(_f);
        // Línea fina del juego en vez del separador de ImGui.
        let p = ui.cursor_screen_pos();
        let w = ui.content_region_avail()[0];
        if self
            .icons
            .as_ref()
            .is_some_and(|i| i.draw(ui, SEP_LINE, [p[0], p[1] + 1.0], [w, 3.0]))
        {
            ui.dummy([w, 8.0]);
        } else {
            ui.separator();
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

        // Columnas, como Relic Rites: la selección a la izquierda y la lista a la derecha.
        let [x0, y0] = ui.cursor_screen_pos();
        let avail = ui.content_region_avail();
        let gap = 24.0;
        let left_w = (avail[0] * 0.42).max(340.0);
        let right_x = x0 + left_w + gap;
        let right_w = avail[0] - left_w - gap;

        // Encabezado izquierdo: murk y cantidad.
        ui.text_colored(GOLD, format!("Murk  {murk}"));
        ui.same_line_with_spacing(0.0, 28.0);
        ui.text_disabled(format!("{}: {}", tr("Reliquias", "Relics"), relics.len()));
        // Encabezado derecho, arriba de la lista (como el de la grilla del juego): texto a
        // buscar, filtros (4) y orden (5).
        ui.same_line();
        ui.set_cursor_screen_pos([right_x, y0]);
        ui.set_next_item_width(right_w * 0.42);
        ui.input_text("##filtro", &mut self.filter)
            .hint(tr("Buscar por nombre o efecto", "Search by name or effect"))
            .build();
        let n = self.filters.active();
        let filter_label = match n {
            0 => tr("Filtrar", "Filter").to_string(),
            n => format!("{} ({n})", tr("Filtrar", "Filter")),
        };
        let sort_label = format!("{}: {}", tr("Orden", "Sort"), self.sort.label());
        let (mut open, mut next) = (false, false);
        {
            let icons = self.icons.as_ref();
            ui.same_line_with_spacing(0.0, 20.0);
            let (g, k, _) = KEY_FILTER;
            open |= key_prompt(ui, icons, g, k, &filter_label, 1.0);
            ui.same_line_with_spacing(0.0, 20.0);
            let (g, k, _) = KEY_SORT;
            next |= key_prompt(ui, icons, g, k, &sort_label, 1.0);
        }
        if open {
            self.show_filters = !self.show_filters;
        }
        if next {
            self.sort = self.sort.next();
        }

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
        ui.dummy([0.0, 6.0]);

        // Paneles: el marco se dibuja atrás y el contenido va en ventanas hijas transparentes,
        // con margen para que los títulos no queden pegados al borde.
        let [px, py] = ui.cursor_screen_pos();
        let panel_h = ui.content_region_avail()[1];
        self.panel_frame(ui, [px, py], [px + left_w, py + panel_h]);
        self.panel_frame(ui, [right_x, py], [right_x + right_w, py + panel_h]);
        let _bg = ui.push_style_color(imgui::StyleColor::ChildBg, [0.0, 0.0, 0.0, 0.0]);
        let _pad = ui.push_style_var(imgui::StyleVar::WindowPadding([20.0, 16.0]));
        ui.child_window("izquierda")
            .size([left_w, panel_h])
            .always_use_window_padding(true)
            .build(|| {
                if self.show_filters {
                    self.draw_filters(ui);
                } else {
                    self.draw_left(ui, &by_handle, preview.as_ref(), preview_busy, forging);
                }
            });
        ui.same_line_with_spacing(0.0, gap);
        ui.child_window("derecha")
            .size([right_w, panel_h])
            .always_use_window_padding(true)
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
                relic_title(ui, self.icons.as_ref(), t);
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
                relic_title(ui, self.icons.as_ref(), d);
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
                relic_title(ui, self.icons.as_ref(), r);
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
                // Ícono de cada efecto, por su texto (salen de la receptora o de la donante).
                let icon_of: HashMap<&str, i32> = [self.target, self.donor]
                    .into_iter()
                    .flatten()
                    .filter_map(|h| by_handle.get(&h))
                    .flat_map(|r| {
                        r.effects
                            .iter()
                            .map(String::as_str)
                            .zip(r.effect_icons.iter().copied())
                    })
                    .collect();
                let text = ui.style_color(imgui::StyleColor::Text);
                for line in msg.lines() {
                    match line.strip_prefix(EFFECT_LINE) {
                        Some(e) => {
                            let color = if Some(&e.to_string()) == moved {
                                GOLD
                            } else {
                                text
                            };
                            self.effect_line(ui, icon_of.get(e).copied(), color, e);
                        }
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
            let mut list = list;
            self.sort.sort(&mut list);
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
            let mut list = list;
            self.sort.sort(&mut list);
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
        let mut list = list;
        self.sort.sort(&mut list);
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
        self.filters.accepts(r) && (f.is_empty() || r.label().to_lowercase().contains(&f))
    }

    /// Subtítulo dentro de un panel (como "Characteristics" en el filtro del juego).
    fn subsection(&self, ui: &Ui, title: &str) {
        ui.spacing();
        ui.text_colored([0.78, 0.80, 0.88, 1.0], title);
        let p = ui.cursor_screen_pos();
        let w = ui.content_region_avail()[0];
        if !self
            .icons
            .as_ref()
            .is_some_and(|i| i.draw(ui, SEP_LINE, [p[0], p[1]], [w, 2.0]))
        {
            ui.separator();
        }
        ui.dummy([w, 4.0]);
    }

    /// El filtro ocupa el panel izquierdo (tecla 4), como en Relic Rites.
    fn draw_filters(&mut self, ui: &Ui) {
        self.section(ui, tr("Filtro", "Filter"));
        hint(
            ui,
            tr(
                "Marcá lo que querés ver. En cada grupo, sin nada marcado se muestra todo.",
                "Check what you want to see. In each group, with nothing checked everything is shown.",
            ),
        );
        ui.spacing();
        ui.checkbox(tr("Favoritas", "Favorites"), &mut self.filters.favorites);
        self.subsection(ui, tr("Características", "Characteristics"));
        let colors = [
            tr("Roja", "Red"),
            tr("Azul", "Blue"),
            tr("Amarilla", "Yellow"),
            tr("Verde", "Green"),
        ];
        for (i, name) in colors.iter().enumerate() {
            ui.checkbox(name, &mut self.filters.colors[i]);
        }
        self.subsection(ui, tr("Tamaño", "Size"));
        let sizes = [
            tr("Chica", "Small"),
            tr("Mediana", "Medium"),
            tr("Grande", "Large"),
        ];
        for (i, name) in sizes.iter().enumerate() {
            ui.checkbox(name, &mut self.filters.sizes[i]);
        }
        self.subsection(ui, tr("Tipo", "Type"));
        ui.checkbox(tr("Reliquia", "Relic"), &mut self.filters.kinds[0]);
        ui.checkbox(
            tr("Reliquia de las profundidades", "Depths Relic"),
            &mut self.filters.kinds[1],
        );
    }

    /// Panel al estilo de Relic Rites: fondo oscuro difuminado y la línea blanca con brillo
    /// arriba (sprites del juego; si no están, un fondo y una línea simples).
    fn panel_frame(&self, ui: &Ui, min: [f32; 2], max: [f32; 2]) {
        let (w, h) = (max[0] - min[0], max[1] - min[1]);
        let icons = self.icons.as_ref();
        // El fondo tiene los bordes difuminados: se agranda para que la parte pareja cubra
        // el panel.
        let (ex, ey) = (w * 0.06, h * 0.06);
        // Base pareja: tapa Relic Rites, que sigue dibujándose detrás.
        ui.get_window_draw_list()
            .add_rect(min, max, [0.02, 0.03, 0.05, 0.62])
            .filled(true)
            .build();
        if !icons.is_some_and(|i| {
            i.draw(
                ui,
                PANEL_BG,
                [min[0] - ex, min[1] - ey],
                [w + ex * 2.0, h + ey * 2.0],
            )
        }) {
            ui.get_window_draw_list()
                .add_rect(min, max, [0.03, 0.04, 0.07, 0.55])
                .filled(true)
                .build();
        }
        let line_h = (w / icons.map_or(1.0, |i| i.aspect(PANEL_LINE))).min(56.0);
        if !icons.is_some_and(|i| i.draw(ui, PANEL_LINE, [min[0], min[1] - 2.0], [w, line_h])) {
            ui.get_window_draw_list()
                .add_line(min, [max[0], min[1]], [0.80, 0.82, 0.90, 0.35])
                .build();
        }
    }

    /// Un efecto de la vista previa: su ícono (si hay) y el texto, que baja de línea si no entra.
    fn effect_line(&self, ui: &Ui, icon: Option<i32>, color: [f32; 4], text: &str) {
        let p = ui.cursor_screen_pos();
        let h = ui.text_line_height();
        let drawn = self.icons.as_ref().is_some_and(|icons| {
            icon.is_some_and(|id| icons.draw(ui, &status_icon(id), [p[0] + 8.0, p[1]], [h, h]))
        });
        ui.dummy([if drawn { h + 8.0 } else { 14.0 }, h]);
        ui.same_line();
        colored_wrapped(ui, color, text);
    }

    /// Lista seleccionable de reliquias, con el ícono de cada una. Devuelve el handle
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
                // Con íconos: la imagen de la reliquia a la izquierda (muestra color, tamaño y
                // si es Deep, como en el juego) y el ícono de cada efecto antes del texto.
                let icons = self.icons.as_ref();
                let relic_icon = icons.filter(|i| i.has(&item_icon(r.icon)));
                let pic = if relic_icon.is_some() { 60.0 } else { 0.0 };
                let name_x = if relic_icon.is_some() {
                    x + pic + 8.0
                } else {
                    x + 20.0
                };
                let effect_x = if relic_icon.is_some() {
                    name_x + line + 6.0
                } else {
                    x + 32.0
                };
                let wrap_w = (x + width - effect_x - 6.0).max(80.0);
                // (sangría, color, texto, ícono del efecto en su primera línea)
                let mut effect_lines: Vec<(f32, [f32; 4], String, Option<i32>)> = Vec::new();
                for (i, e) in r.effects.iter().enumerate() {
                    let color = if r.forged.get(i) == Some(&true) {
                        GOLD
                    } else {
                        EFFECT
                    };
                    for (j, l) in wrap_text(ui, e, wrap_w).into_iter().enumerate() {
                        // Las continuaciones van un poco más adentro.
                        let indent = if j == 0 { 0.0 } else { 14.0 };
                        let icon = if j == 0 {
                            r.effect_icons.get(i).copied()
                        } else {
                            None
                        };
                        effect_lines.push((indent, color, l, icon));
                    }
                }
                let lines = 1 + effect_lines.len().max(1);
                let row_h = (line * lines as f32 + 6.0).max(pic + 4.0);
                if ui
                    .selectable_config(format!("##{id}{:X}", r.handle))
                    .selected(selected.contains(&r.handle))
                    .size([0.0, row_h])
                    .build()
                {
                    clicked = Some(r.handle);
                }
                if let Some(i) = relic_icon {
                    i.draw(
                        ui,
                        &item_icon(r.icon),
                        [x, y + (row_h - pic) * 0.5],
                        [pic, pic],
                    );
                } else {
                    color_mark(ui, [x + 4.0, y + 3.0 + line * 0.5], r.color);
                }
                let y = y + 3.0;
                for (i, (.., icon)) in effect_lines.iter().enumerate() {
                    if let (Some(icons), Some(id)) = (icons, icon) {
                        let at = [name_x, y + line * (i + 1) as f32];
                        icons.draw(ui, &status_icon(*id), at, [line, line]);
                    }
                }
                let draw = ui.get_window_draw_list();
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
                for (i, (indent, color, l, _)) in effect_lines.iter().enumerate() {
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
            let icon = r.effect_icons.get(i).copied();
            if let Some(icons) = &self.icons {
                let p = ui.cursor_screen_pos();
                let h = ui.text_line_height();
                if icon.is_some_and(|id| icons.draw(ui, &status_icon(id), p, [h, h])) {
                    ui.dummy([h, h]);
                    ui.same_line();
                }
            }
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

/// Ícono (o marca de color, si no hay íconos) y nombre de una reliquia, con sus forjas si tiene.
fn relic_title(ui: &Ui, icons: Option<&Icons>, r: &RelicView) {
    let p = ui.cursor_screen_pos();
    let h = ui.text_line_height();
    let s = (h * 1.6).round();
    if icons.is_some_and(|i| i.draw(ui, &item_icon(r.icon), [p[0] - 4.0, p[1]], [s, s])) {
        ui.dummy([s - 4.0, s]);
        ui.same_line();
        ui.set_cursor_screen_pos([p[0] + s + 2.0, p[1] + (s - h) * 0.5]);
    } else {
        color_mark(ui, [p[0], p[1] + h * 0.5], r.color);
        ui.dummy([14.0, h]);
        ui.same_line();
    }
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
                ctx.fonts().add_font(&[source(data, 21.0)]);
                // Para el control "F7 :Forge relics", del tamaño de los controles del juego a 1080p.
                self.prompt_font = Some(TitleFont(ctx.fonts().add_font(&[source(data, 24.0)])));
            }
            None => {
                crate::log_event("Menú: no encontré Palatino ni Georgia, uso la fuente de ImGui")
            }
        }
        if let Some(data) = load(&["palab.ttf", "georgiab.ttf"]) {
            self.title_font = Some(TitleFont(ctx.fonts().add_font(&[source(data, 24.0)])));
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        // La forja vive dentro de Relic Rites: fuera de ese menú no se muestra nada.
        if !self.game.relic_rites_open() {
            if self.visible.swap(false, Ordering::Relaxed) {
                self.reset_selection();
            }
            crate::input_block::set_menu_open(false);
            return;
        }
        let hint_clicked = !self.visible.load(Ordering::Relaxed)
            && draw_hint(
                ui,
                self.prompt_font.or(self.title_font),
                self.icons.as_ref(),
                crate::relic_menu::panel(),
            );
        if ui.is_key_pressed(TOGGLE_KEY) || hint_clicked {
            let now = !self.visible.load(Ordering::Relaxed);
            self.visible.store(now, Ordering::Relaxed);
            if now {
                self.refresh();
            }
        }
        if self.visible.load(Ordering::Relaxed) {
            self.draw(ui);
        }
        // Con la forja abierta, el juego no recibe teclado ni mouse (ver `input_block`).
        crate::input_block::set_menu_open(self.visible.load(Ordering::Relaxed));
    }

    fn before_render<'a>(
        &'a mut self,
        ctx: &mut imgui::Context,
        render_context: &'a mut dyn hudhook::RenderContext,
    ) {
        // Con el menú abierto, ImGui dibuja su propio cursor (el juego suele esconder el de Windows).
        ctx.io_mut().mouse_draw_cursor = self.visible.load(Ordering::Relaxed);
        // El atlas de íconos llega de otro hilo; se sube a la GPU una sola vez.
        if self.icons.is_none() {
            let atlas = self
                .pending_icons
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            if let Some(atlas) = atlas {
                match render_context.load_texture(&atlas.rgba, atlas.width, atlas.height) {
                    Ok(texture) => {
                        self.icons = Some(Icons {
                            texture,
                            size: [atlas.width as f32, atlas.height as f32],
                            rects: atlas.rects,
                        })
                    }
                    Err(e) => crate::log_event(&format!("Íconos: no pude subir la textura: {e:?}")),
                }
            }
        }
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
    let mut favorites = HashSet::new();
    for (_, entry) in game.inventory_entries() {
        order.insert(entry[0], entry[3]);
        if is_favorite(&entry) {
            favorites.insert(entry[0]);
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
            let effect_icons = relic
                .slots
                .iter()
                .filter_map(|s| s.effect)
                .map(|e| data.effects.get(&e).map_or(-1, |i| i.icon_id))
                .collect();
            let kind = data.relic_types.get(&r.item_id);
            RelicView {
                handle: r.handle,
                forged,
                name: kind.map_or("?".into(), |t| t.name.clone()),
                color: kind.map_or(u8::MAX, |t| t.color),
                icon: kind.map_or(-1, |t| t.icon_id),
                size,
                deep,
                effects,
                effect_icons,
                locked: locks.get(&r.handle).map(|p| p.join(", ")),
                order: order.get(&r.handle).copied().unwrap_or(u32::MAX),
                favorite: favorites.contains(&r.handle),
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
    // Líneas blancas apagadas, como los bordes de los menús del juego (antes, doradas).
    let gold_line = [0.80, 0.82, 0.90, 0.22];
    for (c, v) in [
        (C::WindowBg, [0.06, 0.07, 0.10, 0.94]),
        (C::ChildBg, [0.0, 0.0, 0.0, 0.0]),
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
