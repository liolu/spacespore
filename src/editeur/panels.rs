//! Interface de l'éditeur (E1) : outils et couleurs à gauche, onglets en haut, modèle à droite,
//! barre d'état en bas, et les fenêtres « Nouveau modèle », « Bibliothèque », « Renommer »,
//! « Étiquettes ». L'interface du jeu est masquée pendant l'édition.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use super::edit::{self, Tool, RACES, TAG_GROUPS};
use super::format::{Material, Model, ModelKind, ShipCategory, CHARACTER_GRID, MAX_FILE_BYTES, OTHER_MAX_GRID};
use super::{library_dir, list_models, save_model, AppState, Editor, Overlay};

const PANEL: Color = Color::srgba(0.05, 0.06, 0.1, 0.94);
const BUTTON: Color = Color::srgb(0.1, 0.13, 0.2);
const ACCENT: Color = Color::srgb(0.35, 0.65, 1.0);
const ON: Color = Color::srgb(1.0, 0.8, 0.3);
const TEXT: Color = Color::srgb(0.88, 0.92, 1.0);
const DIM: Color = Color::srgb(0.6, 0.66, 0.75);

/// Un panneau de l'éditeur : la souris y est hors de la vue 3D.
#[derive(Component)]
pub struct Blocks;

/// Racine de l'interface de l'éditeur.
#[derive(Component)]
pub struct EditorUi;

#[derive(Component)]
pub enum Live {
    Status,
    Info,
    Rename,
}

/// Ce que fait un bouton.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum Act {
    Tool(Tool),
    Mirror,
    Grid,
    Undo,
    Redo,
    Focus,
    Color(usize),
    Tab(usize),
    CloseTab,
    New,
    Library,
    Save,
    Quit,
    Rename,
    Tags,
    CloseOverlay,
    NewKind(ModelKind),
    NewRace(usize),
    NewCategory(ShipCategory),
    NewSize(u32),
    Create,
    Open(usize),
    Duplicate(usize),
    Delete(usize),
    Tag(&'static str),
    RenameOk,
    Import,
}

fn button(p: &mut ChildBuilder, label: &str, act: Act, on: bool) {
    p.spawn((
        Button,
        Node { padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)), border: UiRect::all(Val::Px(2.0)), ..default() },
        BackgroundColor(BUTTON),
        BorderColor(if on { ON } else { ACCENT }),
        BorderRadius::all(Val::Px(5.0)),
        act,
    ))
    .with_child((Text::new(label), TextFont { font_size: 14.0, ..default() }, TextColor(TEXT)));
}

fn label(p: &mut ChildBuilder, s: &str, size: f32, color: Color) {
    p.spawn((Text::new(s), TextFont { font_size: size, ..default() }, TextColor(color)));
}

fn wrap(p: &mut ChildBuilder, f: impl FnOnce(&mut ChildBuilder)) {
    p.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), ..default() }).with_children(f);
}

fn panel(node: Node) -> impl Bundle {
    (node, BackgroundColor(PANEL), BorderRadius::all(Val::Px(8.0)), Interaction::default(), Blocks)
}

/// Couleur d'affichage d'une entrée de palette.
fn swatch(e: &super::format::PaletteEntry) -> Color {
    Color::srgb_u8(e.rgb[0], e.rgb[1], e.rgb[2])
}

/// Reconstruit l'interface quand l'état change (onglet, outil, fenêtre...).
pub fn rebuild(mut commands: Commands, mut editor: ResMut<Editor>, old: Query<Entity, With<EditorUi>>) {
    if !editor.ui_dirty {
        return;
    }
    editor.ui_dirty = false;
    for e in &old {
        commands.entity(e).try_despawn_recursive();
    }
    let ed = &*editor;
    commands
        .spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(60), EditorUi))
        .with_children(|root| {
            // ── Gauche : outils et couleurs ──
            root.spawn(panel(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(8.0),
                bottom: Val::Px(40.0),
                width: Val::Px(250.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(10.0)),
                overflow: Overflow::clip_y(),
                ..default()
            }))
            .with_children(|p| {
                label(p, "EDITEUR", 20.0, ACCENT);
                wrap(p, |w| {
                    for t in Tool::ALL {
                        button(w, t.name(), Act::Tool(t), ed.tool == t);
                    }
                });
                wrap(p, |w| {
                    button(w, "Miroir (X)", Act::Mirror, ed.mirror);
                    button(w, "Grille (G)", Act::Grid, ed.grid);
                    button(w, "Cadrer (F)", Act::Focus, false);
                });
                wrap(p, |w| {
                    button(w, "Annuler (Ctrl+Z)", Act::Undo, false);
                    button(w, "Retablir (Ctrl+Y)", Act::Redo, false);
                });
                label(p, &format!("Couleur : {} ({})", hex(&ed.color.rgb), ed.color.material.name()), 14.0, TEXT);
                p.spawn((Node { width: Val::Px(226.0), height: Val::Px(22.0), ..default() }, BackgroundColor(swatch(&ed.color)), BorderRadius::all(Val::Px(4.0))));
                label(p, "Couleurs (Maj+clic : pipette)", 13.0, DIM);
                wrap(p, |w| {
                    for (i, e) in ed.palette.iter().enumerate() {
                        let mark = match e.material {
                            Material::Mate => "",
                            Material::Metal => "M",
                            Material::Verre => "V",
                            Material::Lumineuse => "L",
                        };
                        w.spawn((
                            Button,
                            Node { width: Val::Px(22.0), height: Val::Px(22.0), border: UiRect::all(Val::Px(2.0)), justify_content: JustifyContent::Center, ..default() },
                            BackgroundColor(swatch(e)),
                            BorderColor(if *e == ed.color { ON } else { Color::srgba(0.0, 0.0, 0.0, 0.5) }),
                            Act::Color(i),
                        ))
                        .with_child((Text::new(mark), TextFont { font_size: 11.0, ..default() }, TextColor(Color::srgba(0.0, 0.0, 0.0, 0.8))));
                    }
                });
            });

            // ── Haut : onglets et fichiers ──
            root.spawn(panel(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(266.0),
                right: Val::Px(276.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            }))
            .with_children(|p| {
                for (i, d) in ed.docs.iter().enumerate() {
                    let star = if d.dirty { " *" } else { "" };
                    button(p, &format!("{}{star}", d.model.name), Act::Tab(i), i == ed.current);
                }
                if !ed.docs.is_empty() {
                    button(p, "Fermer l'onglet", Act::CloseTab, false);
                }
                button(p, "Nouveau", Act::New, false);
                button(p, "Bibliotheque", Act::Library, false);
                button(p, "Enregistrer (Ctrl+S)", Act::Save, false);
                button(p, "Retour au jeu (Echap)", Act::Quit, false);
            });

            // ── Droite : le modèle ──
            root.spawn(panel(Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                top: Val::Px(8.0),
                width: Val::Px(260.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            }))
            .with_children(|p| {
                label(p, "MODELE", 18.0, ACCENT);
                p.spawn((Text::new(""), TextFont { font_size: 14.0, ..default() }, TextColor(TEXT), Live::Info));
                if !ed.docs.is_empty() {
                    wrap(p, |w| {
                        button(w, "Renommer", Act::Rename, false);
                        button(w, "Etiquettes", Act::Tags, false);
                    });
                }
            });

            // ── Bas : état ──
            root.spawn(panel(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                right: Val::Px(8.0),
                bottom: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                ..default()
            }))
            .with_child((Text::new(""), TextFont { font_size: 14.0, ..default() }, TextColor(TEXT), Live::Status));

            if let Some(o) = &ed.overlay {
                overlay(root, ed, o);
            }
        });
}

fn hex(rgb: &[u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// Fenêtre au centre de l'écran.
fn overlay(root: &mut ChildBuilder, ed: &Editor, o: &Overlay) {
    root.spawn(panel(Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Px(70.0),
        width: Val::Px(760.0),
        margin: UiRect::left(Val::Px(-380.0)),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(10.0),
        padding: UiRect::all(Val::Px(16.0)),
        border: UiRect::all(Val::Px(2.0)),
        ..default()
    }))
    .insert(BorderColor(ACCENT))
    .with_children(|p| match o {
        Overlay::New => {
            label(p, "NOUVEAU MODELE", 20.0, ACCENT);
            if ed.welcome {
                label(p, "Bienvenue ! Cree ton personnage : choisis ta race, puis construis-le bloc par bloc.", 15.0, ON);
            }
            wrap(p, |w| {
                for k in ModelKind::ALL {
                    button(w, k.name(), Act::NewKind(k), ed.new_kind == k);
                }
            });
            match ed.new_kind {
                ModelKind::Personnage => {
                    label(p, &format!("Race (grille {} x {} x {}) - squelette et animations : E5", CHARACTER_GRID.x, CHARACTER_GRID.y, CHARACTER_GRID.z), 14.0, DIM);
                    wrap(p, |w| {
                        for (i, r) in RACES.iter().enumerate() {
                            button(w, r, Act::NewRace(i), ed.new_race == Some(i));
                        }
                    });
                }
                ModelKind::Vaisseau => {
                    label(p, "Categorie (taille de la grille)", 14.0, DIM);
                    wrap(p, |w| {
                        for c in ShipCategory::ALL {
                            button(w, &format!("{} {}³", c.name(), c.grid()), Act::NewCategory(c), ed.new_category == c);
                        }
                    });
                }
                ModelKind::Autre => {
                    label(p, &format!("Taille de la grille (jusqu'a {m}³)", m = OTHER_MAX_GRID), 14.0, DIM);
                    wrap(p, |w| {
                        for s in [8, 16, 32, 48, 64] {
                            button(w, &format!("{s}³"), Act::NewSize(s), ed.new_size == s);
                        }
                    });
                }
            }
            wrap(p, |w| {
                button(w, "Creer", Act::Create, true);
                if !ed.docs.is_empty() || !ed.welcome {
                    button(w, "Annuler", Act::CloseOverlay, false);
                }
            });
        }
        Overlay::Library => {
            label(p, &format!("BIBLIOTHEQUE ({})", library_dir().display()), 18.0, ACCENT);
            if ed.library.is_empty() {
                label(p, "Aucun modele enregistre. Pour importer : copiez des JSON de Pixel World dans saves/import/.", 14.0, DIM);
            }
            for (i, e) in ed.library.iter().enumerate().take(18) {
                p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                    label(r, &format!("{}  -  {}, {} voxels, {:.1} Ko", e.name, e.kind.name(), e.voxels, e.bytes as f64 / 1024.0), 14.0, TEXT);
                    button(r, "Ouvrir", Act::Open(i), false);
                    button(r, "Dupliquer", Act::Duplicate(i), false);
                    let sure = ed.pending_delete == Some(i);
                    button(r, if sure { "Confirmer la suppression" } else { "Supprimer" }, Act::Delete(i), sure);
                });
            }
            if ed.library.len() > 18 {
                label(p, &format!("... et {} autres", ed.library.len() - 18), 13.0, DIM);
            }
            wrap(p, |w| {
                button(w, "Importer Pixel World (saves/import/)", Act::Import, false);
                button(w, "Fermer", Act::CloseOverlay, false);
            });
        }
        Overlay::Rename(_) => {
            label(p, "RENOMMER (Entree : valider, Echap : annuler)", 18.0, ACCENT);
            p.spawn((Text::new(""), TextFont { font_size: 18.0, ..default() }, TextColor(ON), Live::Rename));
            wrap(p, |w| {
                button(w, "Valider", Act::RenameOk, true);
                button(w, "Annuler", Act::CloseOverlay, false);
            });
        }
        Overlay::Tags => {
            label(p, "ETIQUETTES", 18.0, ACCENT);
            let tags = ed.doc().map(|d| d.model.tags.clone()).unwrap_or_default();
            for (group, list) in TAG_GROUPS {
                label(p, group, 14.0, DIM);
                wrap(p, |w| {
                    for t in list.iter() {
                        button(w, t, Act::Tag(t), tags.iter().any(|x| x == t));
                    }
                });
            }
            wrap(p, |w| button(w, "Fermer", Act::CloseOverlay, false));
        }
    });
}

/// Textes qui changent à chaque image.
pub fn live_texts(mut editor: ResMut<Editor>, mut q: Query<(&Live, &mut Text)>) {
    editor.refresh_size();
    let ed = &*editor;
    for (which, mut t) in &mut q {
        let s = match which {
            Live::Status => {
                let at = match (ed.tool, ed.hover) {
                    (Tool::Add, (_, Some(p))) | (_, (Some(p), _)) => format!("case {} {} {}   ", p.x, p.y, p.z),
                    _ => String::new(),
                };
                let msg = if ed.message.is_empty() { "Clic gauche : outil   Clic droit : tourner   Clic molette : deplacer   Molette : zoom   Fleches : onglets".to_string() } else { ed.message.clone() };
                format!("{at}{msg}")
            }
            Live::Info => match ed.doc() {
                Some(d) => {
                    let m = &d.model;
                    let what = match (m.kind, &m.race, m.category) {
                        (ModelKind::Personnage, Some(r), _) => format!("Personnage - {r}"),
                        (ModelKind::Vaisseau, _, Some(c)) => format!("Vaisseau - {}", c.name()),
                        (k, _, _) => k.name().to_string(),
                    };
                    let size = ed.size_bytes.map_or("?".into(), |b| format!("{:.2}", b as f64 / (1024.0 * 1024.0)));
                    let full = ed.size_bytes.is_some_and(|b| b > MAX_FILE_BYTES);
                    format!(
                        "{}\n{what}\nGrille {} x {} x {}\n{} voxels, {} couleurs\nFichier : {size} / 10 Mo{}\nEtiquettes : {}\n{}",
                        m.name,
                        m.size.x,
                        m.size.y,
                        m.size.z,
                        m.voxels.count(),
                        m.palette.len(),
                        if full { "  TROP LOURD" } else { "" },
                        if m.tags.is_empty() { "aucune".into() } else { m.tags.join(", ") },
                        d.path.as_ref().map_or("pas encore enregistre".into(), |p| p.file_name().map_or(String::new(), |f| f.to_string_lossy().to_string())),
                    )
                }
                None => "Aucun modele ouvert : Nouveau ou Bibliotheque.".into(),
            },
            Live::Rename => match &ed.overlay {
                Some(Overlay::Rename(s)) => format!("{s}_"),
                _ => String::new(),
            },
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// Saisie du nouveau nom (clavier logique : AZERTY compris).
pub fn typing(mut events: EventReader<KeyboardInput>, mut editor: ResMut<Editor>) {
    let Some(Overlay::Rename(mut name)) = editor.overlay.clone() else {
        events.clear();
        return;
    };
    let mut done = None;
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Character(c) => {
                if name.chars().count() < 40 {
                    name.push_str(c);
                }
            }
            Key::Space => name.push(' '),
            Key::Backspace => {
                name.pop();
            }
            Key::Enter => done = Some(true),
            Key::Escape => done = Some(false),
            _ => {}
        }
    }
    match done {
        Some(true) => editor.rename(name),
        Some(false) => {
            editor.overlay = None;
            editor.ui_dirty = true;
            editor.escape_used = true;
        }
        None => editor.overlay = Some(Overlay::Rename(name)),
    }
}

/// Échap : ferme la fenêtre ouverte, sinon revient au jeu.
pub fn escape(keys: Res<ButtonInput<KeyCode>>, mut editor: ResMut<Editor>, mut next: ResMut<NextState<AppState>>) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if std::mem::take(&mut editor.escape_used) {
        return;
    }
    if editor.overlay.is_some() && !(editor.welcome && editor.docs.is_empty()) {
        editor.overlay = None;
        editor.ui_dirty = true;
    } else {
        next.set(AppState::Jeu);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn actions(interactions: Query<(&Interaction, &Act), Changed<Interaction>>, mut editor: ResMut<Editor>, mut next: ResMut<NextState<AppState>>) {
    for (i, act) in &interactions {
        if *i != Interaction::Pressed {
            continue;
        }
        let ed = &mut *editor;
        ed.ui_dirty = true;
        if !matches!(act, Act::Delete(_)) {
            ed.pending_delete = None;
        }
        match *act {
            Act::Tool(t) => ed.tool = t,
            Act::Mirror => ed.mirror = !ed.mirror,
            Act::Grid => ed.grid = !ed.grid,
            Act::Undo => ed.undo(),
            Act::Redo => ed.redo(),
            Act::Focus => ed.focus(),
            Act::Color(k) => {
                if let Some(c) = ed.palette.get(k).copied() {
                    ed.set_color(c);
                }
            }
            Act::Tab(k) => ed.select(k),
            Act::CloseTab => ed.close_tab(),
            Act::New => {
                ed.overlay = Some(Overlay::New);
            }
            Act::Library => {
                ed.library = list_models(&library_dir());
                ed.overlay = Some(Overlay::Library);
            }
            Act::Save => ed.save(),
            Act::Quit => next.set(AppState::Jeu),
            Act::Rename => {
                let name = ed.doc().map(|d| d.model.name.clone()).unwrap_or_default();
                ed.overlay = Some(Overlay::Rename(name));
            }
            Act::Tags => ed.overlay = Some(Overlay::Tags),
            Act::CloseOverlay => ed.overlay = None,
            Act::NewKind(k) => ed.new_kind = k,
            Act::NewRace(r) => ed.new_race = Some(r),
            Act::NewCategory(c) => ed.new_category = c,
            Act::NewSize(s) => ed.new_size = s,
            Act::Create => ed.create(),
            Act::Open(k) => ed.open_library(k),
            Act::Duplicate(k) => ed.duplicate(k),
            Act::Delete(k) => ed.delete(k),
            Act::Tag(t) => {
                if let Some(d) = ed.doc_mut() {
                    if let Some(pos) = d.model.tags.iter().position(|x| x == t) {
                        d.model.tags.remove(pos);
                    } else {
                        d.model.tags.push(t.to_string());
                    }
                    d.dirty = true;
                }
            }
            Act::Import => ed.import(),
            Act::RenameOk => {
                if let Some(Overlay::Rename(name)) = ed.overlay.clone() {
                    ed.rename(name);
                }
            }
        }
    }
}

/// Masque l'interface du jeu (on la rend en sortant).
pub fn hide_game_ui(
    mut editor: ResMut<Editor>,
    mut roots: Query<(Entity, &mut Visibility), (With<Node>, Without<Parent>, Without<EditorUi>, Without<TargetCamera>)>,
) {
    for (e, mut v) in &mut roots {
        if *v != Visibility::Hidden {
            if !editor.hidden_ui.contains(&e) {
                editor.hidden_ui.push(e);
            }
            *v = Visibility::Hidden;
        }
    }
}

pub fn show_game_ui(mut editor: ResMut<Editor>, mut q: Query<&mut Visibility, (With<Node>, Without<EditorUi>)>, mut commands: Commands, ui: Query<Entity, With<EditorUi>>) {
    for e in std::mem::take(&mut editor.hidden_ui) {
        if let Ok(mut v) = q.get_mut(e) {
            *v = Visibility::Inherited;
        }
    }
    for e in &ui {
        commands.entity(e).try_despawn_recursive();
    }
}

/// Nom de modèle unique parmi les onglets.
pub fn fresh_name(base: &str, taken: &[String]) -> String {
    let mut k = 1;
    loop {
        let n = format!("{base} {k}");
        if !taken.contains(&n) {
            return n;
        }
        k += 1;
    }
}

impl Editor {
    /// Crée le modèle choisi dans « Nouveau modèle ».
    fn create(&mut self) {
        if self.new_kind == ModelKind::Personnage && self.new_race.is_none() {
            self.say("Choisis d'abord une race.".into());
            return;
        }
        let mut taken: Vec<String> = self.docs.iter().map(|d| d.model.name.clone()).collect();
        taken.extend(list_models(&library_dir()).into_iter().map(|e| e.name));
        let category = (self.new_kind == ModelKind::Vaisseau).then_some(self.new_category);
        let base = match (self.new_kind, self.new_race) {
            (ModelKind::Personnage, Some(r)) => RACES[r].split(" /").next().unwrap_or("Personnage").to_string(),
            (ModelKind::Vaisseau, _) => self.new_category.name().to_string(),
            _ => "Objet".to_string(),
        };
        let mut m = Model::new(&fresh_name(&base, &taken), self.new_kind, category);
        if self.new_kind == ModelKind::Personnage {
            m.race = self.new_race.map(|r| RACES[r].to_string());
            m.tags.push("personnage".into());
        }
        if self.new_kind == ModelKind::Autre {
            m.size = UVec3::splat(self.new_size);
        }
        super::view::open_doc(self, m, None);
        self.overlay = None;
        self.welcome = false;
        self.say("Nouveau modele : clic gauche pour poser des blocs. Ctrl+S pour enregistrer.".into());
    }

    fn open_library(&mut self, k: usize) {
        let Some(e) = self.library.get(k).cloned() else { return };
        if let Some(i) = self.docs.iter().position(|d| d.path.as_ref() == Some(&e.path)) {
            self.select(i);
            self.overlay = None;
            return;
        }
        match std::fs::read(&e.path).map_err(|x| x.to_string()).and_then(|b| Model::from_bytes(&b)) {
            Ok(m) => {
                super::view::open_doc(self, m, Some(e.path.clone()));
                self.overlay = None;
                self.say(format!("Ouvert : {}", e.name));
            }
            Err(x) => self.say(format!("Impossible d'ouvrir {} : {x}", e.name)),
        }
    }

    fn duplicate(&mut self, k: usize) {
        let Some(e) = self.library.get(k).cloned() else { return };
        let r = std::fs::read(&e.path).map_err(|x| x.to_string()).and_then(|b| Model::from_bytes(&b)).and_then(|mut m| {
            m.name = format!("{} (copie)", m.name);
            save_model(&library_dir(), &m)
        });
        self.say(match r {
            Ok(p) => format!("Copie enregistree : {}", p.display()),
            Err(x) => x,
        });
        self.library = list_models(&library_dir());
    }

    fn delete(&mut self, k: usize) {
        if self.pending_delete != Some(k) {
            self.pending_delete = Some(k);
            return;
        }
        self.pending_delete = None;
        let Some(e) = self.library.get(k).cloned() else { return };
        let msg = match std::fs::remove_file(&e.path) {
            Ok(()) => {
                // L'onglet ouvert reste, sans fichier (on peut le réenregistrer)
                for d in &mut self.docs {
                    if d.path.as_ref() == Some(&e.path) {
                        d.path = None;
                        d.dirty = true;
                    }
                }
                format!("Supprime : {}", e.name)
            }
            Err(x) => format!("Suppression impossible : {x}"),
        };
        self.say(msg);
        self.library = list_models(&library_dir());
    }

    fn rename(&mut self, name: String) {
        let name = name.trim().to_string();
        self.overlay = None;
        self.ui_dirty = true;
        self.escape_used = true;
        if name.is_empty() {
            return;
        }
        if let Some(d) = self.doc_mut() {
            d.model.name = name;
            d.dirty = true;
        }
    }

    fn close_tab(&mut self) {
        if self.docs.is_empty() {
            return;
        }
        let d = self.docs.remove(self.current);
        if d.dirty {
            self.say(format!("\"{}\" ferme sans enregistrer les derniers changements.", d.model.name));
        }
        let i = self.current.min(self.docs.len().saturating_sub(1));
        if !self.docs.is_empty() {
            self.select(i);
        }
    }
}

/// Couleur de départ de l'éditeur.
pub fn default_color() -> super::format::PaletteEntry {
    edit::starter_palette()[0]
}
