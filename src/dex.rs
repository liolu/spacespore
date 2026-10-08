// ─────────────────────────────────────────────────────────────────────────
//  Dex des découvertes (0.13.1)
//
//  Chaque astre passé au scanner (touche I) entre dans le dex : toutes ses informations (les
//  sections du scanner, figées à la dernière visite), la date de sa découverte et qui l'a faite, le
//  nombre de visites et d'atterrissages, et une note libre. Le scanner montre l'historique de
//  l'astre ciblé. Panneau : touche K ou bouton « Dex » : onglets par sorte d'astre, recherche,
//  listes qui défilent, note ; export (`saves/.../export/dex-*.json`) et import (`import/dex*.json`,
//  même monde seulement). Sauvegardé dans `dex.json` à côté de `world.json`.
// ─────────────────────────────────────────────────────────────────────────

use std::collections::BTreeMap;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde::{Deserialize, Serialize};

use crate::net_ui::{Field, NetPanel, ACCENT, BG_DARK, TEXT_COLOR, TEXT_DIM};
use crate::settings::GameSettings;
use crate::ui::{CameraTarget, MenuState};

pub const DEX_KEY: KeyCode = KeyCode::KeyK;

pub struct DexPlugin;

impl Plugin for DexPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Dex::load())
            .init_resource::<LastScan>()
            .init_resource::<DexUi>()
            .add_systems(Startup, spawn_dex_button)
            .add_systems(Update, (record_scans, count_landings, save_dex, toggle_dex, dex_buttons, dex_typing, dex_scroll, rebuild_dex).chain());
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Données
// ─────────────────────────────────────────────────────────────────────────

/// Sortes d'astres (onglets).
pub const CATEGORIES: [&str; 5] = ["Etoiles", "Planetes", "Lunes", "Cometes", "Asteroides"];

/// Une section d'informations : titre, lignes (libellé, valeur).
pub type InfoSection = (String, Vec<(String, String)>);

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DexEntry {
    pub key: String,
    pub category: String,
    pub name: String,
    /// Toutes les informations du scanner, à la dernière visite.
    pub sections: Vec<InfoSection>,
    /// Date réelle de la découverte, heure du monde (secondes de jeu), découvreur.
    pub discovered: String,
    pub discovered_clock: f64,
    pub by: String,
    pub visits: u32,
    pub landings: u32,
    pub last_seen: String,
    #[serde(default)]
    pub note: String,
    /// Entrée venue d'un dex importé (de qui).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_from: Option<String>,
}

#[derive(Resource, Default, Serialize, Deserialize)]
pub struct Dex {
    #[serde(default)]
    pub entries: BTreeMap<String, DexEntry>,
    #[serde(skip)]
    dirty: bool,
    #[serde(skip)]
    since: f32,
}

/// Dernier astre lu par le scanner (le scanner l'écrit, le dex l'enregistre).
#[derive(Resource, Default)]
pub struct LastScan {
    pub key: Option<String>,
    pub category: String,
    pub name: String,
    pub sections: Vec<InfoSection>,
    pub serial: u64,
}

impl LastScan {
    pub fn set(&mut self, key: String, category: &str, name: &str, sections: Vec<InfoSection>) {
        self.key = Some(key);
        self.category = category.to_string();
        self.name = name.to_string();
        self.sections = sections;
        self.serial = self.serial.wrapping_add(1);
    }
}

fn now_text() -> String {
    chrono::Local::now().format("%d/%m/%Y %H:%M").to_string()
}

fn dex_path() -> std::path::PathBuf {
    crate::settings::data_dir().join("dex.json")
}

impl Dex {
    fn load() -> Self {
        std::fs::read_to_string(dex_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&mut self) {
        if cfg!(test) {
            return;
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dex_path(), json);
        }
        self.dirty = false;
    }

    /// Un astre vu au scanner : nouvelle découverte, ou une visite de plus (les informations sont
    /// mises à jour).
    pub fn record(&mut self, key: &str, category: &str, name: &str, sections: Vec<InfoSection>, clock: f64, by: &str) -> bool {
        let now = now_text();
        let fresh = !self.entries.contains_key(key);
        let e = self.entries.entry(key.to_string()).or_insert_with(|| DexEntry {
            key: key.to_string(),
            category: category.to_string(),
            discovered: now.clone(),
            discovered_clock: clock,
            by: by.to_string(),
            ..Default::default()
        });
        e.name = name.to_string();
        e.category = category.to_string();
        e.sections = sections;
        e.visits += 1;
        e.last_seen = now;
        self.dirty = true;
        fresh
    }

    /// Section « Historique » du scanner pour cet astre.
    pub fn history(&self, key: &str) -> Option<InfoSection> {
        let e = self.entries.get(key)?;
        let mut rows = vec![
            ("Decouverte".to_string(), format!("{} par {}", e.discovered, e.by)),
            ("Visites".to_string(), format!("{} (derniere : {})", e.visits, e.last_seen)),
            ("Atterrissages".to_string(), e.landings.to_string()),
        ];
        if let Some(from) = &e.imported_from {
            rows.push(("Importe de".to_string(), from.clone()));
        }
        if !e.note.is_empty() {
            let short: String = e.note.lines().next().unwrap_or("").chars().take(80).collect();
            rows.push(("Note".to_string(), short));
        }
        Some(("HISTORIQUE (K : dex)".to_string(), rows))
    }

    /// Entrées d'un onglet (`None` = toutes) qui contiennent la recherche (nom, sorte, note,
    /// informations), triées par nom.
    pub fn filtered(&self, category: Option<&str>, search: &str) -> Vec<&DexEntry> {
        let s = plain(search);
        let mut out: Vec<&DexEntry> = self
            .entries
            .values()
            .filter(|e| category.is_none_or(|c| e.category == c))
            .filter(|e| {
                s.is_empty()
                    || plain(&e.name).contains(&s)
                    || plain(&e.note).contains(&s)
                    || e.sections.iter().any(|(_, rows)| rows.iter().any(|(a, b)| plain(a).contains(&s) || plain(b).contains(&s)))
            })
            .collect();
        out.sort_by(|a, b| plain(&a.name).cmp(&plain(&b.name)));
        out
    }

    /// Fusionne un dex importé : les astres inconnus sont ajoutés (marqués « importé de »), ceux
    /// déjà connus gardent leurs informations, une note importée s'ajoute à la nôtre.
    pub fn merge(&mut self, other: DexFile) -> usize {
        let mut added = 0;
        for mut e in other.entries {
            match self.entries.get_mut(&e.key) {
                Some(mine) => {
                    if !e.note.is_empty() && !mine.note.contains(&e.note) {
                        mine.note = if mine.note.is_empty() { format!("[{}] {}", other.by, e.note) } else { format!("{}\n[{}] {}", mine.note, other.by, e.note) };
                    }
                }
                None => {
                    e.imported_from = Some(other.by.clone());
                    e.visits = 0;
                    e.landings = 0;
                    self.entries.insert(e.key.clone(), e);
                    added += 1;
                }
            }
        }
        self.dirty = true;
        added
    }
}

/// Minuscules sans accents, pour la recherche.
fn plain(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect()
}

/// Fichier d'export (`dex-*.json`).
#[derive(Serialize, Deserialize)]
pub struct DexFile {
    pub format: String,
    pub by: String,
    pub world_seed: u64,
    pub entries: Vec<DexEntry>,
}

// ─────────────────────────────────────────────────────────────────────────
//  Enregistrement
// ─────────────────────────────────────────────────────────────────────────

fn record_scans(mut dex: ResMut<Dex>, last: Res<LastScan>, settings: Res<GameSettings>, clock: Res<crate::world_clock::WorldClock>, mut seen: Local<u64>, mut net: ResMut<crate::net::Net>, time: Res<Time>) {
    if last.serial == *seen {
        return;
    }
    *seen = last.serial;
    let Some(key) = last.key.clone() else { return };
    if dex.record(&key, &last.category, &last.name, last.sections.clone(), clock.secs, &settings.player_name) {
        net.notify(&format!("Nouvelle decouverte : {} ({} au dex, touche K).", last.name, dex.entries.len()), time.elapsed_secs_f64());
    }
}

/// Un atterrissage (à pied sur l'astre) compte dans le dex.
fn count_landings(mut dex: ResMut<Dex>, surface: Res<crate::surface::Surface>, star_q: Query<&crate::planet::StarId, With<crate::planet::StarRoot>>, mut was: Local<bool>) {
    let walking = surface.walker_state().is_some();
    if walking && !*was {
        if let Some(kind) = surface.body() {
            let key = match kind {
                crate::ui::TargetKind::Asteroid(k) => Some(format!("{k:?}")),
                k => {
                    let loaded = matches!(k, crate::ui::TargetKind::Star(id) if star_q.iter().any(|s| s.0 == id));
                    crate::chat_cmd::target_body(&k, loaded).map(|b| b.key())
                }
            };
            if let Some(e) = key.and_then(|k| dex.entries.get_mut(&k)) {
                e.landings += 1;
                dex.dirty = true;
            }
        }
    }
    *was = walking;
}

/// Écrit le dex quelques secondes après un changement.
fn save_dex(time: Res<Time>, mut dex: ResMut<Dex>) {
    // (sans marquer le dex « changé » : le panneau et le scanner ne se reconstruisent pas pour ça)
    let d = dex.bypass_change_detection();
    if !d.dirty {
        d.since = 0.0;
        return;
    }
    d.since += time.delta_secs();
    if d.since > 3.0 {
        d.since = 0.0;
        d.save();
    }
}

// ─────────────────────────────────────────────────────────────────────────
//  Panneau
// ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Typing {
    Search,
    Note,
}

#[derive(Resource, Default)]
pub struct DexUi {
    /// Panneau ouvert (le scanner se cache derrière).
    pub open: bool,
    /// Onglet : `None` = tout, sinon l'indice dans `CATEGORIES`.
    tab: Option<usize>,
    search: String,
    selected: Option<String>,
    typing: Option<Typing>,
    dirty: bool,
    /// Défilement de la liste et des détails (gardé quand le panneau est reconstruit).
    scroll: [f32; 2],
}

#[derive(Component)]
struct DexOpenButton;

#[derive(Component)]
struct DexRoot;

#[derive(Component, Clone, PartialEq)]
enum DexAct {
    Tab(Option<usize>),
    Select(String),
    Search,
    Note,
    Export,
    Import,
    Target,
    Close,
}

#[derive(Component)]
struct DexScroll(usize);

#[derive(Component)]
struct NoteText;

fn spawn_dex_button(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(300.0),
                top: Val::Px(20.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(ACCENT),
            BorderRadius::all(Val::Px(6.0)),
            Button,
            DexOpenButton,
        ))
        .with_child((Text::new("Dex"), TextFont { font_size: 18.0, ..default() }, TextColor(TEXT_COLOR)));
}

fn stop_typing(ui: &mut DexUi, panel: &mut NetPanel) {
    ui.typing = None;
    if panel.focus == Some(Field::Dex) {
        panel.focus = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn toggle_dex(keys: Res<ButtonInput<KeyCode>>, menu: Res<MenuState>, mut panel: ResMut<NetPanel>, mut ui: ResMut<DexUi>, button: Query<&Interaction, (Changed<Interaction>, With<DexOpenButton>)>, time: Res<Time>, dex: Res<Dex>, mut tested: Local<bool>) {
    // Tests : `SPACESPORE_TEST_DEX=<s>` ouvre le dex à cet instant, sur la dernière découverte
    if let Some(at) = std::env::var("SPACESPORE_TEST_DEX").ok().and_then(|v| v.parse::<f32>().ok()) {
        if !*tested && time.elapsed_secs() > at {
            *tested = true;
            ui.open = true;
            ui.selected = dex.entries.values().max_by(|a, b| a.last_seen.cmp(&b.last_seen).then(a.discovered_clock.total_cmp(&b.discovered_clock))).map(|e| e.key.clone());
            ui.dirty = true;
        }
    }
    let clicked = button.iter().any(|i| *i == Interaction::Pressed);
    let key = keys.just_pressed(DEX_KEY) && panel.focus.is_none() && !menu.open;
    if clicked || key {
        ui.open = !ui.open;
        ui.dirty = true;
        stop_typing(&mut ui, &mut panel);
    }
    if ui.open && keys.just_pressed(KeyCode::Escape) && ui.typing.is_none() && panel.focus.is_none() {
        ui.open = false;
        ui.dirty = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn dex_buttons(
    acts: Query<(&Interaction, &DexAct), Changed<Interaction>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut ui: ResMut<DexUi>,
    mut dex: ResMut<Dex>,
    mut panel: ResMut<NetPanel>,
    settings: Res<GameSettings>,
    mut target: ResMut<CameraTarget>,
    mut net: ResMut<crate::net::Net>,
    time: Res<Time>,
) {
    if !ui.open {
        return;
    }
    let now = time.elapsed_secs_f64();
    let mut any = false;
    for (i, act) in &acts {
        if *i != Interaction::Pressed {
            continue;
        }
        any = true;
        match act.clone() {
            DexAct::Tab(t) => {
                ui.tab = t;
                ui.scroll[0] = 0.0;
                stop_typing(&mut ui, &mut panel);
            }
            DexAct::Select(k) => {
                ui.selected = Some(k);
                ui.scroll[1] = 0.0;
                stop_typing(&mut ui, &mut panel);
            }
            DexAct::Search => {
                ui.typing = Some(Typing::Search);
                panel.focus = Some(Field::Dex);
            }
            DexAct::Note => {
                if ui.selected.is_some() {
                    ui.typing = Some(Typing::Note);
                    panel.focus = Some(Field::Dex);
                }
            }
            DexAct::Export => {
                stop_typing(&mut ui, &mut panel);
                net.notify(&export(&dex, &settings), now);
            }
            DexAct::Import => {
                stop_typing(&mut ui, &mut panel);
                net.notify(&import(&mut dex, &settings), now);
            }
            DexAct::Target => {
                if let Some(kind) = ui.selected.as_deref().and_then(target_of) {
                    target.0 = kind;
                    net.notify("Astre vise (si sa distance le permet, le vaisseau y va).", now);
                }
            }
            DexAct::Close => {
                ui.open = false;
                stop_typing(&mut ui, &mut panel);
            }
        }
        ui.dirty = true;
    }
    // Clic ailleurs dans le panneau : on quitte la saisie
    if !any && mouse.just_pressed(MouseButton::Left) && ui.typing.is_some() {
        stop_typing(&mut ui, &mut panel);
        ui.dirty = true;
    }
}

/// L'astre d'une entrée, à viser (étoile, planète, lune).
fn target_of(key: &str) -> Option<crate::ui::TargetKind> {
    use crate::planetgen::live::BodyId;
    Some(match BodyId::parse(key)? {
        BodyId::Star { system, .. } => crate::ui::TargetKind::Star(system as usize),
        BodyId::Planet { system, index } => crate::ui::TargetKind::Planet(system as usize * 1000 + index as usize),
        BodyId::Moon { system, planet, index } => crate::ui::TargetKind::Moon(system as usize * 1000 + planet as usize, index as usize),
    })
}

fn export(dex: &Dex, settings: &GameSettings) -> String {
    let file = DexFile { format: "spacespore-dex".into(), by: settings.player_name.clone(), world_seed: settings.world_seed, entries: dex.entries.values().cloned().collect() };
    let dir = crate::settings::data_dir().join("export");
    let _ = std::fs::create_dir_all(&dir);
    let name: String = settings.player_name.chars().filter(|c| c.is_alphanumeric()).collect();
    let path = dir.join(format!("dex-{}-{}.json", if name.is_empty() { "joueur".into() } else { name }, chrono::Local::now().format("%Y%m%d-%H%M")));
    match serde_json::to_string_pretty(&file).map_err(|e| e.to_string()).and_then(|j| std::fs::write(&path, j).map_err(|e| e.to_string())) {
        Ok(()) => format!("Dex exporte ({} astres) : {}", file.entries.len(), path.display()),
        Err(e) => format!("Export du dex impossible : {e}"),
    }
}

fn import(dex: &mut Dex, settings: &GameSettings) -> String {
    let dir = crate::settings::data_dir().join("import");
    let Ok(rd) = std::fs::read_dir(&dir) else { return format!("Pour importer, copiez un fichier dex-*.json dans {}", dir.display()) };
    let (mut files, mut added, mut other_world) = (0, 0, 0);
    for p in rd.filter_map(|e| e.ok().map(|e| e.path())) {
        let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !(name.starts_with("dex") && name.ends_with(".json")) {
            continue;
        }
        let Some(file) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<DexFile>(&t).ok()) else { continue };
        if file.world_seed != settings.world_seed {
            other_world += 1;
            continue;
        }
        files += 1;
        added += dex.merge(file);
    }
    match (files, other_world) {
        (0, 0) => format!("Aucun fichier dex-*.json dans {}", dir.display()),
        (_, 0) => format!("Dex importe : {added} nouveaux astres ({files} fichier(s))."),
        _ => format!("Dex importe : {added} nouveaux astres ; {other_world} fichier(s) d'un autre monde ignore(s)."),
    }
}

/// Saisie de la recherche et de la note.
fn dex_typing(mut events: EventReader<KeyboardInput>, keys: Res<ButtonInput<KeyCode>>, mut ui: ResMut<DexUi>, mut dex: ResMut<Dex>, mut panel: ResMut<NetPanel>, mut note_q: Query<&mut Text, With<NoteText>>) {
    let Some(typing) = ui.typing.filter(|_| ui.open) else {
        events.clear();
        return;
    };
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let mut changed = false;
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        let mut text = match typing {
            Typing::Search => ui.search.clone(),
            Typing::Note => ui.selected.as_ref().and_then(|k| dex.entries.get(k)).map(|e| e.note.clone()).unwrap_or_default(),
        };
        match &ev.logical_key {
            Key::Escape => {
                stop_typing(&mut ui, &mut panel);
                ui.dirty = true;
                return;
            }
            Key::Enter if typing == Typing::Search => {
                stop_typing(&mut ui, &mut panel);
                ui.dirty = true;
                return;
            }
            Key::Enter => text.push('\n'),
            Key::Backspace => {
                text.pop();
            }
            Key::Space => text.push(' '),
            Key::Character(s) if ctrl => {
                if s.eq_ignore_ascii_case("v") {
                    if let Some(clip) = arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()) {
                        text.push_str(&clip);
                    }
                }
            }
            Key::Character(s) => text.extend(s.chars().filter(|c| !c.is_control())),
            _ => continue,
        }
        changed = true;
        match typing {
            Typing::Search => {
                ui.search = text.chars().take(60).collect();
                ui.scroll[0] = 0.0;
                ui.dirty = true;
            }
            Typing::Note => {
                if let Some(e) = ui.selected.clone().and_then(|k| dex.entries.get_mut(&k)) {
                    e.note = text.chars().take(2000).collect();
                }
                dex.dirty = true;
            }
        }
    }
    // La note se met à jour sur place (pas de reconstruction : le défilement ne bouge pas)
    if changed && typing == Typing::Note {
        let note = ui.selected.as_ref().and_then(|k| dex.entries.get(k)).map(|e| e.note.clone()).unwrap_or_default();
        for mut t in &mut note_q {
            t.0 = format!("{note}|");
        }
    }
}

/// La molette fait défiler la liste ou les détails sous la souris.
fn dex_scroll(mut wheel: EventReader<bevy::input::mouse::MouseWheel>, windows: Query<&Window, With<PrimaryWindow>>, mut ui: ResMut<DexUi>, mut q: Query<(&ComputedNode, &GlobalTransform, &mut ScrollPosition, &DexScroll)>) {
    let lines: f32 = wheel.read().map(crate::ui::wheel_lines).sum();
    if !ui.open {
        return;
    }
    let cursor = windows.get_single().ok().and_then(|w| w.cursor_position());
    for (cn, gt, mut s, which) in &mut q {
        // La mise en page ramène la position dans le contenu : on garde la vraie
        ui.scroll[which.0] = s.offset_y;
        if lines == 0.0 {
            continue;
        }
        let inv = cn.inverse_scale_factor();
        let size = cn.size() * inv;
        let lo = gt.translation().truncate() * inv - size * 0.5;
        if cursor.is_some_and(|c| c.cmpge(lo).all() && c.cmple(lo + size).all()) {
            s.offset_y = (s.offset_y - lines * 48.0).max(0.0);
            ui.scroll[which.0] = s.offset_y;
        }
    }
}

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s.into()), TextFont { font_size: size, ..default() }, TextColor(color))
}

fn button(p: &mut ChildBuilder, label: &str, act: DexAct, on: bool) {
    p.spawn((
        Button,
        Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), border: UiRect::all(Val::Px(2.0)), ..default() },
        BackgroundColor(Color::srgb(0.1, 0.13, 0.2)),
        BorderColor(if on { Color::srgb(1.0, 0.8, 0.3) } else { ACCENT }),
        BorderRadius::all(Val::Px(5.0)),
        act,
    ))
    .with_child(text(label, 14.0, TEXT_COLOR));
}

/// Reconstruit le panneau quand quelque chose change (onglet, recherche, choix...).
fn rebuild_dex(mut commands: Commands, mut ui: ResMut<DexUi>, dex: Res<Dex>, roots: Query<Entity, With<DexRoot>>) {
    // Une nouvelle découverte s'affiche si le panneau est ouvert
    if !ui.dirty && !(ui.open && dex.is_changed() && ui.typing != Some(Typing::Note)) {
        return;
    }
    ui.dirty = false;
    for e in &roots {
        if let Some(ec) = commands.get_entity(e) { ec.despawn_recursive(); }
    }
    if !ui.open {
        return;
    }
    let category = ui.tab.map(|t| CATEGORIES[t]);
    let list = dex.filtered(category, &ui.search);
    let selected = ui.selected.as_ref().and_then(|k| dex.entries.get(k));
    let caret = |on: bool| if on { "|" } else { "" };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(8.0),
                right: Val::Percent(8.0),
                top: Val::Percent(8.0),
                bottom: Val::Percent(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(BG_DARK),
            BorderColor(ACCENT),
            BorderRadius::all(Val::Px(8.0)),
            GlobalZIndex(500),
            Interaction::default(),
            DexRoot,
        ))
        .with_children(|root| {
            // En-tête
            root.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                r.spawn(text(format!("DEX DES DECOUVERTES  ({} astres)", dex.entries.len()), 20.0, ACCENT));
                r.spawn(Node { flex_grow: 1.0, ..default() });
                button(r, "Exporter", DexAct::Export, false);
                button(r, "Importer", DexAct::Import, false);
                button(r, "Fermer (K)", DexAct::Close, false);
            });
            // Onglets
            root.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), ..default() }).with_children(|r| {
                button(r, &format!("Tout ({})", dex.entries.len()), DexAct::Tab(None), ui.tab.is_none());
                for (i, c) in CATEGORIES.iter().enumerate() {
                    let n = dex.entries.values().filter(|e| e.category == *c).count();
                    button(r, &format!("{c} ({n})"), DexAct::Tab(Some(i)), ui.tab == Some(i));
                }
            });
            // Corps : liste à gauche, détails à droite
            root.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(12.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }).with_children(|body| {
                body.spawn(Node { width: Val::Percent(38.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), min_height: Val::Px(0.0), ..default() }).with_children(|left| {
                    // Recherche
                    let typing = ui.typing == Some(Typing::Search);
                    left.spawn((
                        Button,
                        Node { padding: UiRect::all(Val::Px(6.0)), border: UiRect::all(Val::Px(2.0)), ..default() },
                        BackgroundColor(Color::srgb(0.04, 0.05, 0.08)),
                        BorderColor(if typing { ACCENT } else { TEXT_DIM }),
                        BorderRadius::all(Val::Px(4.0)),
                        DexAct::Search,
                    ))
                    .with_child({
                        let (s, c) = if ui.search.is_empty() && !typing { ("Rechercher (nom, type, note, minerai...)".to_string(), TEXT_DIM) } else { (format!("{}{}", ui.search, caret(typing)), TEXT_COLOR) };
                        text(s, 14.0, c)
                    });
                    left.spawn(text(format!("{} resultat(s)", list.len()), 12.0, TEXT_DIM));
                    // Liste qui défile
                    left.spawn((
                        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), flex_grow: 1.0, min_height: Val::Px(0.0), overflow: Overflow::scroll_y(), ..default() },
                        ScrollPosition { offset_x: 0.0, offset_y: ui.scroll[0] },
                        Interaction::default(),
                        DexScroll(0),
                    ))
                    .with_children(|l| {
                        // (un bloc qui ne rétrécit pas : dans un conteneur qui défile, les enfants
                        // seraient sinon tassés)
                        l.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), flex_shrink: 0.0, width: Val::Percent(100.0), ..default() }).with_children(|l| {
                        if list.is_empty() {
                            l.spawn(text(if dex.entries.is_empty() { "Aucune decouverte : visez un astre (touche I : scanner)." } else { "Rien ne correspond." }, 14.0, TEXT_DIM));
                        }
                        for e in &list {
                            let on = ui.selected.as_deref() == Some(e.key.as_str());
                            let kind = e.sections.first().and_then(|(_, rows)| rows.first()).map(|(_, v)| v.clone()).unwrap_or_default();
                            l.spawn((
                                Button,
                                Node { flex_direction: FlexDirection::Column, padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::left(Val::Px(3.0)), flex_shrink: 0.0, ..default() },
                                BackgroundColor(if on { Color::srgb(0.12, 0.18, 0.3) } else { Color::srgb(0.07, 0.08, 0.12) }),
                                BorderColor(if on { Color::srgb(1.0, 0.8, 0.3) } else { ACCENT }),
                                DexAct::Select(e.key.clone()),
                            ))
                            .with_children(|b| {
                                b.spawn(text(format!("{}{}", e.name, if e.note.is_empty() { "" } else { "  *" }), 15.0, TEXT_COLOR));
                                b.spawn(text(format!("{}  -  {}  -  {}", e.category, kind, e.discovered), 11.0, TEXT_DIM));
                            });
                        }
                        });
                    });
                });
                // Détails qui défilent
                body.spawn((
                    Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), overflow: Overflow::scroll_y(), min_height: Val::Px(0.0), padding: UiRect::all(Val::Px(8.0)), ..default() },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
                    ScrollPosition { offset_x: 0.0, offset_y: ui.scroll[1] },
                    Interaction::default(),
                    DexScroll(1),
                ))
                .with_children(|d| {
                    d.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_shrink: 0.0, width: Val::Percent(100.0), ..default() }).with_children(|d| {
                    let Some(e) = selected else {
                        d.spawn(text("Choisissez un astre dans la liste.", 15.0, TEXT_DIM));
                        return;
                    };
                    d.spawn(text(e.name.clone(), 20.0, ACCENT));
                    d.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), ..default() }).with_children(|r| {
                        if target_of(&e.key).is_some() {
                            button(r, "Viser", DexAct::Target, false);
                        }
                    });
                    let mut sections: Vec<InfoSection> = dex.history(&e.key).into_iter().collect();
                    sections.extend(e.sections.iter().cloned());
                    for (title, rows) in sections {
                        d.spawn((text(title, 12.0, Color::srgb(0.45, 0.75, 1.0)), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }));
                        for (label, value) in rows {
                            d.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), flex_shrink: 0.0, ..default() }).with_children(|r| {
                                r.spawn((text(label, 13.0, TEXT_DIM), Node { width: Val::Px(120.0), flex_shrink: 0.0, ..default() }));
                                r.spawn((text(value, 13.0, TEXT_COLOR), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
                            });
                        }
                    }
                    // Note
                    d.spawn((text("NOTE (cliquez pour ecrire, Entree = nouvelle ligne, Echap = fini)", 12.0, Color::srgb(0.45, 0.75, 1.0)), Node { margin: UiRect::top(Val::Px(10.0)), ..default() }));
                    let typing = ui.typing == Some(Typing::Note);
                    d.spawn((
                        Button,
                        Node { padding: UiRect::all(Val::Px(8.0)), border: UiRect::all(Val::Px(2.0)), min_height: Val::Px(80.0), flex_shrink: 0.0, ..default() },
                        BackgroundColor(Color::srgb(0.04, 0.05, 0.08)),
                        BorderColor(if typing { ACCENT } else { TEXT_DIM }),
                        BorderRadius::all(Val::Px(4.0)),
                        DexAct::Note,
                    ))
                    .with_child((
                        {
                            let (s, c) = if e.note.is_empty() && !typing { ("(aucune note)".to_string(), TEXT_DIM) } else { (format!("{}{}", e.note, caret(typing)), TEXT_COLOR) };
                            text(s, 14.0, c)
                        },
                        NoteText,
                    ));
                    });
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, cat: &str, name: &str, note: &str) -> (String, String, String, String) {
        (key.into(), cat.into(), name.into(), note.into())
    }

    /// Découverte, visites, historique, recherche, onglets, export / import.
    #[test]
    fn the_dex_records_searches_and_merges() {
        let mut dex = Dex::default();
        let sec = vec![("IDENTITE".to_string(), vec![("Type".to_string(), "geante gazeuse".to_string())])];
        assert!(dex.record("s1.p3", "Planetes", "Vega 4", sec.clone(), 10.0, "Moi"));
        assert!(!dex.record("s1.p3", "Planetes", "Vega 4", sec.clone(), 20.0, "Moi"));
        assert!(dex.record("s1.e0", "Etoiles", "Vega", Vec::new(), 30.0, "Moi"));
        let e = &dex.entries["s1.p3"];
        assert_eq!((e.visits, e.by.as_str(), e.discovered_clock), (2, "Moi", 10.0));
        let h = dex.history("s1.p3").unwrap();
        assert!(h.1.iter().any(|(a, b)| a == "Visites" && b.starts_with('2')));
        // Recherche dans les informations, sans accents ; onglets
        assert_eq!(dex.filtered(None, "GAZEUSE").len(), 1);
        assert_eq!(dex.filtered(Some("Etoiles"), "").len(), 1);
        assert_eq!(dex.filtered(None, "").len(), 2);
        dex.entries.get_mut("s1.e0").unwrap().note = "Très belle étoile".into();
        assert_eq!(dex.filtered(None, "tres belle").len(), 1);
        // Import : les inconnus sont ajoutés, les notes s'ajoutent
        let (k, c, n, note) = entry("s2.p0", "Planetes", "Altair 1", "monde de lave");
        let other = DexFile {
            format: "spacespore-dex".into(),
            by: "Ami".into(),
            world_seed: 1,
            entries: vec![
                DexEntry { key: k, category: c, name: n, note, visits: 5, ..Default::default() },
                DexEntry { key: "s1.e0".into(), name: "Vega".into(), note: "vue de loin".into(), ..Default::default() },
            ],
        };
        assert_eq!(dex.merge(other), 1);
        let a = &dex.entries["s2.p0"];
        assert_eq!((a.imported_from.as_deref(), a.visits), (Some("Ami"), 0));
        assert!(dex.entries["s1.e0"].note.contains("[Ami] vue de loin"));
        assert!(matches!(target_of("s1.p3"), Some(crate::ui::TargetKind::Planet(1003))));
    }
}
